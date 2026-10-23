//! Comparator cost for mixed canvas and world transparent draws.
//!
//! `master` is the `117a8df5` farthest-corner comparator (`camera_distance_sq`, no canvas key).
//! `recompute` rebuilds the current plane-distance key on every compare, including the canvas
//! stem-name match. That recompute path is an earlier revision of this change, not master.
//! `precomputed` is the production comparator (`WorldMeshDrawItem::transparent_depth_key`).
//! The clone that feeds each sample sits in `iter_batched` setup and is not part of the median.
//!
//! ```text
//! cargo bench -p renderide-bench --bench transparent_sort
//! ```
//!
//! Sample size 10, 1s warmup, 2s measurement, matching the other CPU benches.

use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use renderide_bench::{TRANSPARENT_SORT_1K, TRANSPARENT_SORT_10K, TransparentSortWorkload};

fn criterion() -> Criterion {
    Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2))
}

fn transparent_sort(c: &mut Criterion) {
    let one_k = TransparentSortWorkload::mixed(TRANSPARENT_SORT_1K);
    let ten_k = TransparentSortWorkload::mixed(TRANSPARENT_SORT_10K);
    let mut group = c.benchmark_group("transparent_sort");
    group.throughput(Throughput::Elements(TRANSPARENT_SORT_1K as u64));
    group.bench_function("master_1k", |b| {
        b.iter_batched(
            || one_k.clone(),
            |batch| black_box(batch.sort_master()),
            BatchSize::LargeInput,
        );
    });
    group.bench_function("recompute_1k", |b| {
        b.iter_batched(
            || one_k.clone(),
            |batch| black_box(batch.sort_recomputing_depth()),
            BatchSize::LargeInput,
        );
    });
    group.bench_function("precomputed_1k", |b| {
        b.iter_batched(
            || one_k.clone(),
            |batch| black_box(batch.sort_precomputed()),
            BatchSize::LargeInput,
        );
    });
    group.throughput(Throughput::Elements(TRANSPARENT_SORT_10K as u64));
    group.bench_function("master_10k", |b| {
        b.iter_batched(
            || ten_k.clone(),
            |batch| black_box(batch.sort_master()),
            BatchSize::LargeInput,
        );
    });
    group.bench_function("recompute_10k", |b| {
        b.iter_batched(
            || ten_k.clone(),
            |batch| black_box(batch.sort_recomputing_depth()),
            BatchSize::LargeInput,
        );
    });
    group.bench_function("precomputed_10k", |b| {
        b.iter_batched(
            || ten_k.clone(),
            |batch| black_box(batch.sort_precomputed()),
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

criterion_group! {
    name = sort;
    config = criterion();
    targets = transparent_sort
}
criterion_main!(sort);
