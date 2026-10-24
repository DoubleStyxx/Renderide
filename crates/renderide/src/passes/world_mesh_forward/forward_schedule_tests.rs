use super::{
    forward_intersection_raster_needed, transparent_sequence::forward_transparent_sequence_needed,
};
use crate::world_mesh::{DrawGroup, InstancePlan, WorldMeshPhase};

/// Empty helper plans skip their raster pass even after opaque work records successfully.
#[test]
fn helper_raster_needed_requires_matching_groups() {
    let empty = InstancePlan::default();
    assert!(!forward_intersection_raster_needed(true, &empty));
    assert!(!forward_transparent_sequence_needed(true, &empty));

    let mut intersect = InstancePlan::default();
    intersect
        .phase_mut(WorldMeshPhase::Intersection)
        .push(DrawGroup {
            representative_draw_idx: 0,
            instance_range: 0..1,
            material_packet_idx: 0,
        });
    assert!(forward_intersection_raster_needed(true, &intersect));
    assert!(!forward_intersection_raster_needed(false, &intersect));

    let mut transparent = InstancePlan::default();
    transparent
        .phase_mut(WorldMeshPhase::Transparent)
        .push(DrawGroup {
            representative_draw_idx: 0,
            instance_range: 0..1,
            material_packet_idx: 0,
        });
    assert!(forward_transparent_sequence_needed(true, &transparent));
    assert!(!forward_transparent_sequence_needed(false, &transparent));
}
