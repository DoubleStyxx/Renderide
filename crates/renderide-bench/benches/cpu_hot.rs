//! Criterion batches for transform math, frustum culling, world matrices, buffer-pool reuse,
//! custom-material uniform packing, bind-group cache churn, and keyword variant cache lookup.
//!
//! `cargo bench -p renderide-bench` runs this file. Pass `--features gpu` to compile the GPU
//! group as well. That group does not open a device; add adapter benches there when they exist.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use renderide_bench::{
    BIND_GROUP_BATCH, BUFFER_POOL_SLOTS, BindGroupChurnWorkload, BufferPoolWorkload, CULL_BATCH,
    CullWorkload, KeywordVariantCacheWorkload, MATERIAL_UNIFORM_BATCH,
    MaterialUniformUpdateWorkload, SHADOW_CHURN_CASTERS, SHADOW_CHURN_LIGHTS, ShadowChurnWorkload,
    TRANSFORM_BATCH, TransformWorkload, WORLD_MATRIX_NODES, WorldMatrixWorkload,
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
    let mut reverse = WorldMatrixWorkload::reverse_chain(WORLD_MATRIX_NODES);
    let _ = reverse.recompute();
    group.bench_function("reverse_chain_recompute", |b| {
        b.iter(|| black_box(reverse.recompute()));
    });
    let mut history = WorldMatrixWorkload::standard();
    let _ = history.recompute();
    let dense_4096: Vec<usize> = (0..WORLD_MATRIX_NODES).collect();
    let sparse: Vec<usize> = (0..64).collect();
    let _ = history.publish_motion_history(&dense_4096);
    group.bench_function("motion_history_publish", |b| {
        b.iter(|| black_box(history.publish_motion_history(&dense_4096)));
    });
    let mut wide = WorldMatrixWorkload::forest(100_000);
    let _ = wide.recompute();
    let dense_100k: Vec<usize> = (0..100_000).collect();
    let _ = wide.publish_motion_history(&sparse);
    group.bench_function("motion_history_publish_100k_sparse", |b| {
        b.iter(|| black_box(wide.publish_motion_history(&sparse)));
    });
    let _ = wide.publish_motion_history(&dense_100k);
    group.bench_function("motion_history_publish_100k_dense", |b| {
        b.iter(|| black_box(wide.publish_motion_history(&dense_100k)));
    });
    group.finish();
}

fn material_uniforms(c: &mut Criterion) {
    let mut workload = MaterialUniformUpdateWorkload::standard();
    let mut group = c.benchmark_group("material_uniforms");
    group.throughput(Throughput::Elements(MATERIAL_UNIFORM_BATCH as u64));
    group.bench_function("property_change_pack", |b| {
        b.iter(|| black_box(workload.pack_changed_properties()));
    });
    group.throughput(Throughput::Elements(1));
    group.bench_function("one_material_repack", |b| {
        b.iter(|| black_box(workload.repack_one()));
    });
    group.finish();
}

fn bind_groups(c: &mut Criterion) {
    let workload = BindGroupChurnWorkload::standard();
    let mut group = c.benchmark_group("bind_groups");
    group.throughput(Throughput::Elements(BIND_GROUP_BATCH as u64));
    group.bench_function("cache_hit", |b| {
        b.iter(|| black_box(workload.cache_hits()));
    });
    group.bench_function("miss_rebuild", |b| {
        b.iter(|| black_box(workload.miss_rebuild()));
    });
    group.finish();
}

fn keyword_variants(c: &mut Criterion) {
    let mut workload = KeywordVariantCacheWorkload::standard();
    let mut group = c.benchmark_group("keyword_variants");
    group.throughput(Throughput::Elements(1));
    group.bench_function("cache_hit", |b| {
        b.iter(|| black_box(workload.cache_hit()));
    });
    group.bench_function("cache_miss", |b| {
        b.iter(|| black_box(workload.cache_miss()));
    });
    group.bench_function("compose", |b| {
        b.iter(|| black_box(workload.compose()));
    });
    group.finish();
}

fn shadow_churn(c: &mut Criterion) {
    let mut workload = ShadowChurnWorkload::standard();
    // The first plan fills the caches. Measured iterations are the steady repaint.
    let _ = workload.prepare();
    let mut group = c.benchmark_group("shadow_churn");
    group.throughput(Throughput::Elements(
        (SHADOW_CHURN_CASTERS.saturating_mul(SHADOW_CHURN_LIGHTS)) as u64,
    ));
    group.bench_function("static_repaint", |b| {
        b.iter(|| black_box(workload.prepare()));
    });
    group.bench_function("cold_plan", |b| {
        b.iter_batched(
            ShadowChurnWorkload::standard,
            |mut sample| black_box(sample.prepare()),
            criterion::BatchSize::LargeInput,
        );
    });
    let mut revision_workload = ShadowChurnWorkload::standard_with_production_revisions();
    let _ = revision_workload.prepare();
    group.bench_function("static_repaint_revisions", |b| {
        b.iter(|| black_box(revision_workload.prepare()));
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

criterion_group!(
    cpu,
    transforms,
    culling,
    world_matrices,
    buffer_pool,
    material_uniforms,
    bind_groups,
    keyword_variants,
    shadow_churn
);

#[cfg(feature = "gpu")]
criterion_group!(gpu_benches, gpu);

#[cfg(not(feature = "gpu"))]
criterion_main!(cpu);

#[cfg(feature = "gpu")]
criterion_main!(cpu, gpu_benches);
