//! Criterion microbenchmarks for Renderide CPU hot paths. Run with
//! `cargo bench -p renderide-bench`.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use glam::{Mat4, Vec3};
use renderide_bench::{frustum_planes, make_transforms, sphere_visible};
use renderide_shared::default_entity_pool::DefaultEntityPool;
use renderide_shared::memory_packer::MemoryPacker;
use renderide_shared::memory_unpacker::MemoryUnpacker;

const SIZES: [usize; 3] = [1_000, 10_000, 100_000];

fn transforms_compose(c: &mut Criterion) {
    let mut g = c.benchmark_group("transform_compose");
    for n in SIZES {
        let xs = make_transforms(n);
        let parent = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let mut out = Vec::with_capacity(n);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                out.clear();
                out.extend(xs.iter().map(|t| {
                    parent * Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.position)
                }));
                black_box(&out);
            });
        });
    }
    g.finish();
}

fn frustum_cull(c: &mut Criterion) {
    let mut g = c.benchmark_group("frustum_cull_spheres");
    let proj = Mat4::perspective_rh(1.2, 16.0 / 9.0, 0.1, 500.0);
    let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 150.0), Vec3::ZERO, Vec3::Y);
    let planes = frustum_planes(proj * view);
    for n in SIZES {
        let xs = make_transforms(n);
        let mut visible: Vec<u32> = Vec::with_capacity(n);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                visible.clear();
                for (i, t) in xs.iter().enumerate() {
                    if sphere_visible(&planes, t.position, t.scale.max_element()) {
                        visible.push(i as u32);
                    }
                }
                black_box(visible.len());
            });
        });
    }
    g.finish();
}

fn ipc_transform_rows(c: &mut Criterion) {
    let mut g = c.benchmark_group("ipc_transform_rows");
    for n in SIZES {
        // Host SHM transform row layout: position(3) + scale(3) + rotation(4) f32s = 40 bytes.
        let xs: Vec<[f32; 10]> = make_transforms(n)
            .iter()
            .map(|t| {
                let (p, s, r) = (t.position, t.scale, t.rotation);
                [p.x, p.y, p.z, s.x, s.y, s.z, r.x, r.y, r.z, r.w]
            })
            .collect();
        let mut buf = vec![0u8; n * size_of::<[f32; 10]>() + 64];
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::new("pack", n), &n, |b, _| {
            b.iter(|| {
                let mut p = MemoryPacker::new(&mut buf);
                p.write_value_list(Some(&xs));
                black_box(p.remaining_len());
            });
        });
        g.bench_with_input(BenchmarkId::new("unpack", n), &n, |b, _| {
            let mut pool = DefaultEntityPool;
            b.iter(|| {
                let mut u = MemoryUnpacker::new(&buf, &mut pool);
                let rows: Vec<[f32; 10]> = u.read_value_list().expect("decode");
                assert_eq!(rows.len(), n);
                black_box(rows.len());
            });
        });
    }
    g.finish();
}

/// Demonstrates the cost the renderer avoids by reusing per-frame scratch buffers.
fn scratch_reuse(c: &mut Criterion) {
    let mut g = c.benchmark_group("per_frame_scratch");
    let n = 10_000usize;
    g.bench_function("fresh_vec_per_frame", |b| {
        b.iter(|| {
            let mut v: Vec<[f32; 16]> = Vec::new();
            for i in 0..n {
                v.push([i as f32; 16]);
            }
            black_box(v.len());
        });
    });
    let mut reused: Vec<[f32; 16]> = Vec::new();
    g.bench_function("reused_scratch", |b| {
        b.iter(|| {
            reused.clear();
            for i in 0..n {
                reused.push([i as f32; 16]);
            }
            black_box(reused.len());
        });
    });
    g.finish();
}

criterion_group!(benches, transforms_compose, frustum_cull, ipc_transform_rows, scratch_reuse);
criterion_main!(benches);
