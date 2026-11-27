# Experimental RDNA 1 mesh fallback

Status: reference allocation module only; no runtime mesh emulation is active.
The original native mesh path and unsupported-hardware refusal are unchanged.

Target: Navi 10 / RX 5700 XT, D3D12 SM 6.8, 8 GiB VRAM, no mesh shader tier or GPU upload heap.

The reference layout in victory-gpu/src/mesh_fallback.rs checks per-dispatch vertex and triangle index capacities, arithmetic overflow and memory budget. It does not allocate GPU resources. Its byte count excludes counters, indirect arguments, alignment and per-primitive attributes, which must be budgeted separately in the eventual backend.

Integration milestones:
1. Synthetic compute shader producing triangle geometry, GPU buffers, UAV barriers, and indexed drawing. Validate rendered pixels and overflow handling.
2. Inspect mesh shader IO and lowering in victory-shader/src/io.rs and emit/lower.rs. Add a distinct compute lowering target, preserving workgroup synchronization and output semantics.
3. Add a compute and graphics PSO pair in victory-gpu/src/pipeline/host.rs. Distinguish fallback cache keys from native mesh keys.
4. Route direct and indirect dispatch through the paired pipelines. Preserve bindings, state, ordering, primitive attributes and resource lifetime. Reject unsupported cases explicitly.
5. Measure correctness and timing on RDNA1 before enabling an opt-in runtime switch. No support or 30 FPS claim until verified.

Tests for the reference module: cargo test --locked -p victory-gpu --lib mesh_fallback::tests
Windows GPU integration tests have not been run.
