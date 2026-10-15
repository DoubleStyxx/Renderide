use glam::Vec3;
use rayon::prelude::*;

use crate::shared::RenderBoundingBox;

use super::trail::{TrailPolyline, trail_chunks_by_point_budget_from_trails};
use super::types::PointParticle;

/// Cheap bound scans use larger chunks so scheduling overhead stays amortized.
const PARTICLE_BOUNDS_PARALLEL_CHUNK_POINTS: usize = 4_096;
/// Point or trail point count required before bounds reduction uses Rayon.
const PARTICLE_BOUNDS_PARALLEL_MIN_POINTS: usize = PARTICLE_BOUNDS_PARALLEL_CHUNK_POINTS * 2;

pub(super) fn bounds_for_points(points: &[PointParticle]) -> RenderBoundingBox {
    if particle_bounds_parallel_is_worthwhile(points.len()) {
        return points
            .par_chunks(PARTICLE_BOUNDS_PARALLEL_CHUNK_POINTS)
            .with_min_len(1)
            .map(|chunk| {
                let mut bounds = BoundsAccumulator::default();
                for point in chunk {
                    let radius = point.size.abs().max_element() * 0.5;
                    bounds.include(point.position - Vec3::splat(radius));
                    bounds.include(point.position + Vec3::splat(radius));
                }
                bounds
            })
            .reduce(BoundsAccumulator::default, |mut a, b| {
                a.merge(b);
                a
            })
            .finish();
    }
    let mut bounds = BoundsAccumulator::default();
    for point in points {
        let radius = point.size.abs().max_element() * 0.5;
        bounds.include(point.position - Vec3::splat(radius));
        bounds.include(point.position + Vec3::splat(radius));
    }
    bounds.finish()
}

pub(super) fn bounds_for_trails(trails: &[TrailPolyline]) -> RenderBoundingBox {
    let point_count = trails.iter().map(|trail| trail.points.len()).sum::<usize>();
    let chunks =
        trail_chunks_by_point_budget_from_trails(trails, PARTICLE_BOUNDS_PARALLEL_CHUNK_POINTS);
    if particle_bounds_parallel_is_worthwhile(point_count) && chunks.len() >= 2 {
        return chunks
            .par_iter()
            .with_min_len(1)
            .map(|range| {
                let mut bounds = BoundsAccumulator::default();
                for trail in &trails[range.clone()] {
                    for point in &trail.points {
                        let radius = point.width.abs() * 0.5;
                        bounds.include(point.position - Vec3::splat(radius));
                        bounds.include(point.position + Vec3::splat(radius));
                    }
                }
                bounds
            })
            .reduce(BoundsAccumulator::default, |mut a, b| {
                a.merge(b);
                a
            })
            .finish();
    }
    let mut bounds = BoundsAccumulator::default();
    for trail in trails {
        for point in &trail.points {
            let radius = point.width.abs() * 0.5;
            bounds.include(point.position - Vec3::splat(radius));
            bounds.include(point.position + Vec3::splat(radius));
        }
    }
    bounds.finish()
}

/// Returns whether a cheap bounds reduction has at least two useful chunks.
fn particle_bounds_parallel_is_worthwhile(point_count: usize) -> bool {
    point_count >= PARTICLE_BOUNDS_PARALLEL_MIN_POINTS && rayon::current_num_threads() > 1
}

#[derive(Default)]
struct BoundsAccumulator {
    min: Option<Vec3>,
    max: Option<Vec3>,
}

impl BoundsAccumulator {
    fn include(&mut self, point: Vec3) {
        if !point.is_finite() {
            return;
        }
        self.min = Some(self.min.map_or(point, |min| min.min(point)));
        self.max = Some(self.max.map_or(point, |max| max.max(point)));
    }

    fn merge(&mut self, other: Self) {
        if let Some(min) = other.min {
            self.include(min);
        }
        if let Some(max) = other.max {
            self.include(max);
        }
    }

    fn finish(self) -> RenderBoundingBox {
        match (self.min, self.max) {
            (Some(min), Some(max)) => RenderBoundingBox {
                center: (min + max) * 0.5,
                extents: (max - min).abs() * 0.5,
            },
            _ => RenderBoundingBox {
                center: Vec3::ZERO,
                extents: Vec3::ZERO,
            },
        }
    }
}
