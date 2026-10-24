//! Criterion batches for transform math, frustum culling, world matrices, buffer-pool reuse,
//! custom-material uniform packing, bind-group cache churn, and keyword variant cache lookup.
//!
//! `cargo bench -p renderide-bench` runs this file. Pass `--features gpu` to compile the GPU
//! group as well. That group does not open a device; add adapter benches there when they exist.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use renderide_bench::{
    BIND_GROUP_BATCH, BUFFER_POOL_SLOTS, BindGroupChurnWorkload, BufferPoolWorkload,
    CAMERA_CLEAR_LOAD_OP_CAMERAS, CULL_BATCH, CullWorkload, HYSTERESIS_BATCH,
    HYSTERESIS_HISTORY_COUNTS, HysteresisCullWorkload, HysteresisHistoryWorkload,
    KeywordVariantCacheWorkload, MATERIAL_UNIFORM_BATCH, MaterialUniformUpdateWorkload,
    SHADOW_CHURN_CASTERS, SHADOW_CHURN_LIGHTS, ShadowChurnWorkload, SkinnedGeometryWorkload,
    TRANSFORM_BATCH, TransformWorkload, WORLD_MATRIX_NODES, WorldMatrixWorkload,
    camera_clear_load_op_checksum, camera_clear_load_op_inputs,
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
    let skinned = SkinnedGeometryWorkload::standard();
    let mut group = c.benchmark_group("culling");
    group.throughput(Throughput::Elements(CULL_BATCH as u64));
    group.bench_function("homogeneous_clip_aabb", |b| {
        b.iter(|| black_box(workload.visible_count()));
    });
    group.bench_function("skinned_mesh_world_geometry", |b| {
        b.iter(|| black_box(skinned.fold()));
    });
    group.finish();
}

fn hysteresis_classification(c: &mut Criterion) {
    let workload = HysteresisCullWorkload::standard();
    {
        let mut group = c.benchmark_group("hysteresis_classification");
        group.throughput(Throughput::Elements(
            u64::try_from(HYSTERESIS_BATCH).unwrap_or(0),
        ));
        group.bench_function("decide_visibility", |b| {
            b.iter(|| black_box(workload.kept_count()));
        });
        group.finish();
    }

    let mut history = c.benchmark_group("hysteresis_history");
    for count in HYSTERESIS_HISTORY_COUNTS {
        for moving in [false, true] {
            let mut workload = HysteresisHistoryWorkload::new(count, moving);
            let _ = workload.step();
            let label = if moving { "moving" } else { "stable" };
            history.throughput(Throughput::Elements(u64::try_from(count).unwrap_or(0)));
            history.bench_function(format!("{count}_{label}"), |b| {
                b.iter(|| black_box(workload.step()));
            });
        }
    }
    history.finish();
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

fn camera_clear_load_ops(c: &mut Criterion) {
    // Times the per-view load-op decision only. The full-target Rgba16Float copy is the real
    // cost of a seeded Nothing or Depth camera and is not in this group.
    // Inputs are built once. The timed loop only reads them, and each element is black-boxed
    // inside the checksum so a repeating 128-camera pattern cannot be folded.
    let inputs = camera_clear_load_op_inputs(CAMERA_CLEAR_LOAD_OP_CAMERAS);
    let mut group = c.benchmark_group("camera_clear");
    group.throughput(Throughput::Elements(inputs.len() as u64));
    group.bench_function("load_op_checksum", |b| {
        b.iter(|| black_box(camera_clear_load_op_checksum(black_box(inputs.as_slice()))));
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
    hysteresis_classification,
    world_matrices,
    buffer_pool,
    material_uniforms,
    bind_groups,
    keyword_variants,
    shadow_churn,
    camera_clear_load_ops
);

#[cfg(feature = "gpu")]
criterion_group!(gpu_benches, gpu);

#[cfg(not(feature = "gpu"))]
criterion_main!(cpu);

#[cfg(feature = "gpu")]
criterion_main!(cpu, gpu_benches);
