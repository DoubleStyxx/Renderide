//! Criterion batches for transform math, frustum culling, world matrices, and buffer-pool reuse.
//!
//! `cargo bench -p renderide-bench` runs this file. Pass `--features gpu` to compile the GPU
//! group as well. That group does not open a device; add adapter benches there when they exist.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use renderide_bench::{
    BUFFER_POOL_SLOTS, BufferPoolWorkload, CULL_BATCH, CullWorkload, TRANSFORM_BATCH,
    TransformWorkload, WORLD_MATRIX_NODES, WorldMatrixWorkload,
};

fn transforms(c: &mut Criterion) {
    let workload = TransformWorkload::standard();
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(TRANSFORM_BATCH as u64));
    group.bench_function("trs_and_degenerate_scale", |b| {
        b.iter(|| black_box(workload.fold_matrices()));
    });
    group.finish();
}

fn culling(c: &mut Criterion) {
    let workload = CullWorkload::standard();
    let mut group = c.benchmark_group("culling");
    group.throughput(Throughput::Elements(CULL_BATCH as u64));
    group.bench_function("homogeneous_clip_aabb", |b| {
        b.iter(|| black_box(workload.visible_count()));
    });
    group.finish();
}

fn world_matrices(c: &mut Criterion) {
    let mut workload = WorldMatrixWorkload::standard();
    // Pay the first cache allocation outside the measured loop.
    let _ = workload.recompute();
    let mut group = c.benchmark_group("world_matrices");
    group.throughput(Throughput::Elements(WORLD_MATRIX_NODES as u64));
    group.bench_function("forest_recompute", |b| {
        b.iter(|| black_box(workload.recompute()));
    });
    group.finish();
}

fn buffer_pool(c: &mut Criterion) {
    let mut workload = BufferPoolWorkload::standard();
    let mut group = c.benchmark_group("buffer_pool");
    group.throughput(Throughput::Elements(BUFFER_POOL_SLOTS as u64));
    group.bench_function("free_list_reuse", |b| {
        b.iter(|| black_box(workload.reuse_cycle()));
    });
    group.finish();
}

#[cfg(feature = "gpu")]
fn gpu(_c: &mut Criterion) {
    // Device benches go in this group. Keeping it empty means `--features gpu` still compiles
    // and links without requiring an adapter, while the default bench command stays CPU-only.
}

criterion_group!(cpu, transforms, culling, world_matrices, buffer_pool);

#[cfg(feature = "gpu")]
criterion_group!(gpu_benches, gpu);

#[cfg(not(feature = "gpu"))]
criterion_main!(cpu);

#[cfg(feature = "gpu")]
criterion_main!(cpu, gpu_benches);
