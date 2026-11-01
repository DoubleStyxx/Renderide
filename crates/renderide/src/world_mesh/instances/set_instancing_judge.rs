//! Judge for G0a, host `SetInstancing` eligibility.
//!
//! Judge test lands before the code it judges. This module does not record the
//! opcode and does not change which draws the planner merges.
//!
//! What it judges: G0a SetInstancing eligibility. The host sends one
//! `SetInstancing` row per material. That row's integer is the only payload;
//! the parser does not read a side buffer for this opcode. Zero means
//! instancing is off for the material. Any other integer means instancing is
//! on. The dispatcher currently ignores the integer and only reports that the
//! material instance changed.
//!
//! Exact-count rule, no tolerance. One material, one submesh, `N=64` identical
//! meshes, base-instance supported:
//! - `SetInstancing` true (`property_id` 1): exactly 1 `ForwardOpaque` draw,
//!   covering all 64 instances.
//! - `SetInstancing` false (`property_id` 0): exactly 64 `ForwardOpaque` draws,
//!   each covering one instance.
//!
//! Authored against master `a557b624c489d3b87afe832a1023841ddeeb1159`.
//!
//! Expected-fail: `scene_dsl/warp_pbs.rs` labels one WARP image mismatch
//! (`expected failure (WARP, #212)`). That label is not a general expected-fail
//! switch. It does not run on lavapipe or Metal, and it does not cover a CPU
//! draw-count case. Cargo reports this case as ignored via
//! `#[ignore = "expected-fail until G0a: record SetInstancing"]`.
//! The code change removes the ignore. Until then, running the case fails
//! because `instancing_enabled_for_material` returns `None` and the planner
//! still emits the heuristic group.

use crate::materials::ShaderPermutation;
use crate::materials::host_data::apply_material_set_instancing_for_test;
use crate::world_mesh::draw_prep::sort_draws;
use crate::world_mesh::test_fixtures::{DummyDrawItemSpec, dummy_world_mesh_draw_item};

use super::{WorldMeshPhase, build_plan, build_plan_for_shader_with_instancing_allowlist};

const MESH_COUNT: usize = 64;
const ENABLED_DRAWS: usize = 1;
const MATERIAL_ID: i32 = 41;
const MESH_ASSET_ID: i32 = 7;
const ENABLED_PROPERTY_ID: i32 = 1;
const DISABLED_PROPERTY_ID: i32 = 0;
const AUTHORED_AGAINST_MASTER: &str = "a557b624c489d3b87afe832a1023841ddeeb1159";

struct AllowlistObservation {
    recorded: Option<bool>,
    instance_changed: bool,
    forward_opaque_draws: usize,
    instance_span: u32,
    all_singleton: bool,
}

fn identical_meshes(count: usize, material_id: i32) -> Vec<crate::world_mesh::WorldMeshDrawItem> {
    (0..count)
        .map(|index| {
            dummy_world_mesh_draw_item(DummyDrawItemSpec {
                material_asset_id: material_id,
                property_block: None,
                skinned: false,
                sorting_order: 0,
                mesh_asset_id: MESH_ASSET_ID,
                node_id: i32::try_from(index).unwrap_or(i32::MAX),
                slot_index: 0,
                collect_order: index,
                alpha_blended: false,
            })
        })
        .collect()
}

fn observe(instancing_enabled: bool) -> AllowlistObservation {
    let property_id = if instancing_enabled {
        ENABLED_PROPERTY_ID
    } else {
        DISABLED_PROPERTY_ID
    };
    let (store, instance_changed) =
        apply_material_set_instancing_for_test(MATERIAL_ID, property_id);
    let recorded = store.instancing_enabled_for_material(MATERIAL_ID);
    let mut draws = identical_meshes(MESH_COUNT, MATERIAL_ID);
    sort_draws(&mut draws);
    let plan = build_plan_for_shader_with_instancing_allowlist(
        &draws,
        true,
        ShaderPermutation(0),
        recorded,
    );
    let groups = plan.phase(WorldMeshPhase::ForwardOpaque);
    let instance_span = groups
        .iter()
        .map(|group| group.instance_range.end - group.instance_range.start)
        .sum();
    let all_singleton = groups
        .iter()
        .all(|group| group.instance_range.end - group.instance_range.start == 1);
    AllowlistObservation {
        recorded,
        instance_changed,
        forward_opaque_draws: groups.len(),
        instance_span,
        all_singleton,
    }
}

fn failure_message(enabled: &AllowlistObservation, disabled: &AllowlistObservation) -> String {
    format!(
        "G0a SetInstancing eligibility exact-count rule failed. \
N={MESH_COUNT} identical meshes, one material, one submesh. \
SetInstancing true (property_id {ENABLED_PROPERTY_ID}): recorded={:?} \
forward_opaque_draws={} instance_span={} instance_changed={} \
expected recorded=Some(true) draws={ENABLED_DRAWS} span={MESH_COUNT}. \
SetInstancing false (property_id {DISABLED_PROPERTY_ID}): recorded={:?} \
forward_opaque_draws={} instance_span={} all_singleton={} instance_changed={} \
expected recorded=Some(false) draws={MESH_COUNT} span={MESH_COUNT} all_singleton=true. \
SetInstancing is not recorded, so the planner kept the batch-key/mesh/submesh \
heuristic and emitted {} forward-opaque draw(s) for both flags.",
        enabled.recorded,
        enabled.forward_opaque_draws,
        enabled.instance_span,
        enabled.instance_changed,
        disabled.recorded,
        disabled.forward_opaque_draws,
        disabled.instance_span,
        disabled.all_singleton,
        disabled.instance_changed,
        enabled.forward_opaque_draws,
    )
}

fn allowlist_matches(enabled: &AllowlistObservation, disabled: &AllowlistObservation) -> bool {
    enabled.instance_changed
        && disabled.instance_changed
        && enabled.recorded == Some(true)
        && enabled.forward_opaque_draws == ENABLED_DRAWS
        && enabled.instance_span == MESH_COUNT as u32
        && disabled.recorded == Some(false)
        && disabled.forward_opaque_draws == MESH_COUNT
        && disabled.instance_span == MESH_COUNT as u32
        && disabled.all_singleton
}

/// Judges G0a SetInstancing eligibility.
///
/// Exact-count rule: `N=64` identical meshes, one material, one submesh.
/// `SetInstancing` true emits exactly 1 `ForwardOpaque` draw (instance span 64).
/// `SetInstancing` false emits exactly 64 `ForwardOpaque` draws (span 1 each).
/// No tolerance.
///
/// Authored against master `a557b624c489d3b87afe832a1023841ddeeb1159`.
///
/// The host row integer is the enabled flag (zero off, nonzero on). The store
/// does not keep that integer, so both plans take the heuristic and this fails.
#[test]
#[ignore = "expected-fail until G0a: record SetInstancing"]
fn set_instancing_eligibility_exact_draw_counts() {
    let enabled = observe(true);
    let disabled = observe(false);
    assert!(
        allowlist_matches(&enabled, &disabled),
        "{}",
        failure_message(&enabled, &disabled)
    );
}

/// `None` is the unrecorded flag. It must stay the same plan as [`build_plan`].
#[test]
fn none_allowlist_matches_build_plan() {
    let mut draws = identical_meshes(MESH_COUNT, MATERIAL_ID);
    sort_draws(&mut draws);
    let heuristic = build_plan(&draws, true);
    let via_allowlist =
        build_plan_for_shader_with_instancing_allowlist(&draws, true, ShaderPermutation(0), None);
    assert_eq!(heuristic, via_allowlist);
}

#[test]
fn provenance_names_g0a_exact_counts_and_master() {
    assert_eq!(MESH_COUNT, 64);
    assert_eq!(ENABLED_DRAWS, 1);
    assert_eq!(ENABLED_PROPERTY_ID, 1);
    assert_eq!(DISABLED_PROPERTY_ID, 0);
    assert_eq!(
        AUTHORED_AGAINST_MASTER,
        "a557b624c489d3b87afe832a1023841ddeeb1159"
    );
    assert_eq!(AUTHORED_AGAINST_MASTER.len(), 40);
}
