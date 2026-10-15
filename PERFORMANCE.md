# Frame-time notes

These changes keep the same texels, draw order, and Hi-Z snapshots. They cut staging allocations, device-thread polls, and extra `Queue::submit` calls on the paths that show up in frame time.

## Upload staging

The persistent upload arena keeps 6 mapped slots. A slot is busy for about two frames (in flight, then `map_async`), so three slots allocate a fresh buffer whenever the GPU is a few frames behind. Four overflow slots absorb that case and are reused after their own completion callback. `prepare_staging_buffer` does not call `Device::poll`. `maintain` polls only when every persistent slot is waiting on the GPU and the overflow pool cannot grow or write.

Texture mip uploads and video frames recorded during an asset drain are copied into one staging buffer. Rows are padded to 256 bytes for `copy_buffer_to_texture`; only the tight source bytes are copied into each row. Those copies share one driver submit with the mesh upload batch. Regions that cannot be staged fall back to `Queue::write_texture`. If the queue gate cannot be taken, the bytes stay in the scratch buffer for the next drain.

One-shot staging buffers, used only when both the ring and the pool are saturated, are retained until that submit.

## Hi-Z readback

`hi_z_begin_frame_readback` polls only when some view still has a recorded copy or an in-flight map. The second poll still runs when a map was just started or an older map is still pending, so occlusion uses the same snapshot as the previous two-poll drain. Polls stay outside the per-view Hi-Z lock.

## Submits and binds

The driver thread folds already-queued plain background batches into the next batch, including at most one following present, OpenXR, or waited batch, and never pulls work that was queued after that batch. One `Queue::submit` covers the group. The submit-done counter advances once per merged batch so waiters still observe each token.

The GTAO depth prefilter reuses its bind group when the depth texture, layer count, frame-uniform buffer, params buffer, and five output views are unchanged. The forward pass skips `@group(1)` when the bind-group allocation and dynamic offset match the previous draw. Draw order is unchanged.

## How to verify

```sh
cargo test -p renderide --lib upload_arena
cargo test -p renderide --lib texture_upload
cargo test -p renderide --lib readback_ring
cargo test -p renderide --lib driver_thread
cargo test -p renderide --lib world_mesh_forward
cargo test -p renderide --lib gtao
```

In a Tracy capture of a texture-streaming scene, `temporary_staging_fallbacks` and driver `Queue::submit` calls per frame should drop once the arenas are warm. A still scene and a streaming scene should match the previous pixels and Hi-Z pop-in timing.
