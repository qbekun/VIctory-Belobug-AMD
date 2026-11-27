//! CPU-side reference layout for experimental mesh-to-compute lowering.
//! This is not a GPU execution backend yet.

/// D3D12 dispatch dimensions and the maximum output of each workgroup.
#[derive(Debug, Clone, Copy)]
pub struct MeshLayout {
    pub groups: [u32; 3],
    pub max_vertices: u32,
    pub max_primitives: u32,
    pub vertex_stride: u32,
}

/// Separate vertex and triangle index storage, with checked byte counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshAllocation {
    pub vertices: u64,
    pub indices: u64,
    pub bytes: u64,
}

impl MeshLayout {
    /// Returns None for invalid dimensions, arithmetic overflow or budget exhaustion.
    pub fn allocation(self, budget: u64) -> Option<MeshAllocation> {
        if self.groups.contains(&0)
            || self.groups.iter().any(|&n| n > 65535)
            || self.max_vertices == 0 || self.max_vertices > 256
            || self.max_primitives == 0 || self.max_primitives > 256
            || self.vertex_stride == 0 || self.vertex_stride % 4 != 0
        {
            return None;
        }
        let groups = self.groups.into_iter().try_fold(1u64, |a, b| a.checked_mul(u64::from(b)))?;
        let vertices = groups.checked_mul(u64::from(self.max_vertices))?;
        let indices = groups.checked_mul(u64::from(self.max_primitives))?.checked_mul(3)?;
        // Indexed drawing uses 32-bit indices and a 32-bit index count.
        if vertices > u64::from(u32::MAX) || indices > u64::from(u32::MAX) {
            return None;
        }
        let bytes = vertices.checked_mul(u64::from(self.vertex_stride))?
            .checked_add(indices.checked_mul(4)?)?;
        (bytes <= budget).then_some(MeshAllocation { vertices, indices, bytes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn triangle() -> MeshLayout {
        MeshLayout { groups: [2, 1, 1], max_vertices: 3, max_primitives: 1, vertex_stride: 16 }
    }
    #[test]
    fn bounded_triangle_layout() {
        assert_eq!(triangle().allocation(120), Some(MeshAllocation { vertices: 6, indices: 6, bytes: 120 }));
        assert_eq!(triangle().allocation(119), None);
    }
    #[test]
    fn rejects_invalid_and_oversized_dispatch() {
        assert_eq!(MeshLayout { groups: [0, 1, 1], ..triangle() }.allocation(u64::MAX), None);
        assert_eq!(MeshLayout { groups: [65535; 3], ..triangle() }.allocation(u64::MAX), None);
        assert_eq!(MeshLayout { vertex_stride: 3, ..triangle() }.allocation(u64::MAX), None);
        assert_eq!(MeshLayout { max_vertices: 257, ..triangle() }.allocation(u64::MAX), None);
    }
}

    
/// Conservative reservation for a triangle mesh dispatch with a separate
/// indexed draw argument for each workgroup and an overflow/status word.
/// Does not include D3D12 allocation alignment or primitive attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshReservation {
    pub geometry: MeshAllocation,
    pub argument_bytes: u64,
    pub status_bytes: u64,
    pub total_bytes: u64,
}

impl MeshLayout {
    pub fn reservation(self, budget: u64) -> Option<MeshReservation> {
        let geometry = self.allocation(budget)?;
        let groups = self.groups.into_iter().try_fold(1u64, |a, b| a.checked_mul(u64::from(b)))?;
        let argument_bytes = groups.checked_mul(20)?;
        let status_bytes = 4;
        let total_bytes = geometry.bytes.checked_add(argument_bytes)?.checked_add(status_bytes)?;
        (total_bytes <= budget).then_some(MeshReservation {
            geometry, argument_bytes, status_bytes, total_bytes,
        })
    }

    /// Fixed-capacity disjoint slots, before a GPU compaction pass.
    pub fn group_offsets(self, group: [u32; 3]) -> Option<(u64, u64)> {
        self.allocation(u64::MAX)?;
        if group.into_iter().zip(self.groups).any(|(i, n)| i >= n) {
            return None;
        }
        let [x, y, z] = group.map(u64::from);
        let [gx, gy, _] = self.groups.map(u64::from);
        let linear = (z * gy + y) * gx + x;
        Some((linear * u64::from(self.max_vertices),
              linear * u64::from(self.max_primitives) * 3))
    }
}

#[cfg(test)]
mod reservation_tests {
    use super::*;
    fn layout() -> MeshLayout {
        MeshLayout { groups: [2, 3, 2], max_vertices: 4, max_primitives: 2, vertex_stride: 16 }
    }
    #[test]
    fn budgets_geometry_and_indirect_arguments() {
        let r = layout().reservation(u64::MAX).unwrap();
        assert_eq!(r.geometry.bytes, 1056);
        assert_eq!(r.argument_bytes, 240);
        assert_eq!(r.total_bytes, 1300);
        assert!(layout().reservation(1299).is_none());
        assert_eq!(layout().reservation(1300), Some(r));
    }
    #[test]
    fn disjoint_three_dimensional_group_slots() {
        assert_eq!(layout().group_offsets([0, 0, 0]), Some((0, 0)));
        assert_eq!(layout().group_offsets([1, 2, 1]), Some((44, 66)));
        assert_eq!(layout().group_offsets([2, 0, 0]), None);
    }
}
