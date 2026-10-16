# Performance notes (Oct 15, 2026)

There's no GPU or Resonite host in this environment. `cargo build --release -p renderide` passes,
`renderide-shared` tests pass (202/202), and the CPU benches run. GPU frame-time gains were **not** measured.

## Changes

| File | Change | Why / expected impact |
|---|---|---|
| `crates/renderide-shared/src/packing/memory_packer.rs` | `write_value_list` now writes the whole POD slice with one bounds check and one `memcpy` (`bytemuck::cast_slice`). Before, it reserved space separately for each element. | IPC encode hot path. It's measured **~17x faster** at 1k-10k rows (15.3 us -> 0.88 us at 1k 40-byte rows) and ~6x faster at 100k. The bytes on the wire are identical. On overflow it now writes nothing instead of a partial prefix, and the result was already an error in that case. |
| `crates/renderide/src/passes/post_processing/motion_blur/pipeline.rs`, `motion_blur.rs` | Velocity-pass and blur-resolve bind groups are cached in `BindGroupMap`s, keyed by the identity of the texture/view/UBO and capped at 16. | Removes 2 D2Array `TextureView`s and 2 `create_bind_group` calls per view per frame while motion blur is on. |
| `crates/renderide/src/passes/post_processing/gtao/pipeline.rs`, `gtao/depth_prefilter_pass.rs` | The combined depth-prefilter bind group (8 bindings) and its raw depth view are cached, keyed by the depth texture, layer count, UBOs, and the 5 mip views from the graph (these are already cached per texture, so they stay stable). | Removes 1 view and 1 large bind group per view per frame while GTAO is on. In VR this is per eye-view. |
| `crates/renderide-bench/*` (new), workspace `Cargo.toml` | New Criterion suite: transform composition, sphere/frustum culling, IPC row pack/unpack, and per-frame scratch reuse vs. fresh allocation. A `gpu` feature is reserved for wgpu benches. | Runs with no host or GPU: `cargo bench -p renderide-bench`. |

## Observations
The codebase is already heavily tuned: hashbrown everywhere, parallel instance planning, cached
pipelines, layouts, and most bind groups, Hi-Z bind groups reused, and subresource-view caches.
The remaining per-frame churn shows up in `note_resource_churn!` sites, so the profiler feature
can confirm the two passes fixed here.

## Next steps
- Profile with `--features tracy` on a real scene and check the `note_resource_churn` counters.
- `FrameGpuResources::create_bind_group` rebuilds the `@group(0)` layout every time it rebuilds the
  bind group. Caching the layout once would help. This only happens on cluster or snapshot changes, so it's low priority.
- Hi-Z downsample opens one compute pass per mip. Recording all the mips in a single pass would cut pass overhead,
  but the per-pass timestamp queries need rework first.
- Other `MemoryPacker` and `MemoryUnpacker` per-element loops (enum lists) could use similar bulk paths.

## Build environment note
The box had no C++ toolchain. Here, imgui-sys was built with `CXX=/workspace/.cxx/c++` (zig c++) and
`LIBRARY_PATH=/workspace/.cxx/lib` (zig libc++ exposed as `libstdc++`). A normal dev machine with g++ needs neither.
