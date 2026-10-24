//! Fixed scenes and the control-API script they record.
//!
//! Renderide Systems is designing a dev-only localhost TCP JSON-RPC API whose methods are
//! `camera.set_pose`, `frames.wait`, and `stats.query`. This crate does not open that socket.
//! The runner applies the same method names in process: the pose is written into the scene
//! submission, frame counts come from the CLI, and `stats.query` is the report file.

/// Scenario names the runner accepts when `--scenario` is omitted.
pub const DEFAULT_SCENARIOS: [&str; 2] = ["unlit_sphere", "multi_primitive_unlit"];

/// Extra scenarios selectable with `--scenario`. They are not in the default CI gate set.
#[cfg(test)]
const EXTRA_SCENARIOS: [&str; 7] = [
    "pbs_lit_material_matrix",
    "many_static_meshes",
    "lights_shadows",
    "offscreen_skinned_avatars",
    "onscreen_skinned_avatars",
    "secondary_camera_copy",
    "secondary_camera_double_buffer",
];

/// Transport name recorded in every report. The socket is not implemented.
pub const CONTROL_TRANSPORT: &str = "localhost-tcp-jsonrpc";

/// Method names of the planned control API, in script order.
pub const CONTROL_METHODS: [&str; 3] = ["camera.set_pose", "frames.wait", "stats.query"];

/// One fixed scene.
#[derive(Clone, Copy, Debug)]
pub struct Scenario {
    /// Stable report name.
    pub name: &'static str,
    /// World-space camera position for `camera.set_pose`.
    pub camera_position: [f32; 3],
    /// Camera rotation quaternion, xyzw.
    pub camera_rotation_xyzw: [f32; 4],
}

const UNLIT_SPHERE: Scenario = Scenario {
    name: "unlit_sphere",
    camera_position: [0.0, 0.0, -3.0],
    camera_rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
};

const MULTI_PRIMITIVE_UNLIT: Scenario = Scenario {
    name: "multi_primitive_unlit",
    camera_position: [0.0, 0.0, -4.2],
    camera_rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
};

const PBS_LIT_MATERIAL_MATRIX: Scenario = Scenario {
    name: "pbs_lit_material_matrix",
    camera_position: [0.0, 0.0, -4.0],
    camera_rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
};

const MANY_STATIC_MESHES: Scenario = Scenario {
    name: "many_static_meshes",
    camera_position: [0.0, 0.0, -24.0],
    camera_rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
};

const LIGHTS_SHADOWS: Scenario = Scenario {
    name: "lights_shadows",
    camera_position: [0.0, 6.0, -14.0],
    camera_rotation_xyzw: [0.130_526_2, 0.0, 0.0, 0.991_444_9],
};

/// 16 visible plus 256 behind-the-camera one-bone skinned avatars, all shadow casters.
const OFFSCREEN_SKINNED_AVATARS: Scenario = Scenario {
    name: "offscreen_skinned_avatars",
    camera_position: [0.0, 4.0, -12.0],
    camera_rotation_xyzw: [0.099_833_42, 0.0, 0.0, 0.995_004_2],
};

/// Control for [`OFFSCREEN_SKINNED_AVATARS`]: the same 272 avatars all in view.
const ONSCREEN_SKINNED_AVATARS: Scenario = Scenario {
    name: "onscreen_skinned_avatars",
    camera_position: [0.0, 4.0, -12.0],
    camera_rotation_xyzw: [0.099_833_42, 0.0, 0.0, 0.995_004_2],
};

/// The `unlit_sphere` scene with the main `Skybox` camera plus two enabled secondary cameras
/// (`Depth` clear, `Nothing` clear), each rendering the sphere into its own 512x512 render
/// texture. On fork master since 66421146 both non-clearing secondary cameras seed their target
/// with a full-target `Rgba16Float` previous-colour copy (the `WorldMeshForwardColorPreserve`
/// pass); 0104fee7 is the pre-copy base for an A/B.
const SECONDARY_CAMERA_COPY: Scenario = Scenario {
    name: "secondary_camera_copy",
    camera_position: [0.0, 0.0, -3.0],
    camera_rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
};

/// `secondary_camera_copy` pose with two stacked double-buffered cameras writing one 512² texture.
///
/// Depth at depth 0 is the first writer. Nothing at depth 1 is the later writer. Both set flags
/// `1 | 4` (enabled and double-buffered). Opt-in, same as the other extra scenarios.
const SECONDARY_CAMERA_DOUBLE_BUFFER: Scenario = Scenario {
    name: "secondary_camera_double_buffer",
    camera_position: [0.0, 0.0, -3.0],
    camera_rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
};

/// Thirty shadowed point lights. Opt-in: absent from [`DEFAULT_SCENARIOS`] so the default CI
/// window stays on the unshadowed scenes. The host raises `per_pixel_lights` to 32 and asks for
/// 128-texel shadow faces.
const MANY_SHADOWED_POINT_LIGHTS: Scenario = Scenario {
    name: "many_shadowed_point_lights",
    camera_position: [0.0, 2.2, -5.5],
    camera_rotation_xyzw: [0.174_108_13, 0.0, 0.0, 0.984_726_55],
};

/// Returns the named scenario.
pub fn by_name(name: &str) -> Option<&'static Scenario> {
    match name {
        "unlit_sphere" => Some(&UNLIT_SPHERE),
        "multi_primitive_unlit" => Some(&MULTI_PRIMITIVE_UNLIT),
        "pbs_lit_material_matrix" => Some(&PBS_LIT_MATERIAL_MATRIX),
        "many_static_meshes" => Some(&MANY_STATIC_MESHES),
        "lights_shadows" => Some(&LIGHTS_SHADOWS),
        "many_shadowed_point_lights" => Some(&MANY_SHADOWED_POINT_LIGHTS),
        "offscreen_skinned_avatars" => Some(&OFFSCREEN_SKINNED_AVATARS),
        "onscreen_skinned_avatars" => Some(&ONSCREEN_SKINNED_AVATARS),
        "secondary_camera_copy" => Some(&SECONDARY_CAMERA_COPY),
        "secondary_camera_double_buffer" => Some(&SECONDARY_CAMERA_DOUBLE_BUFFER),
        _ => None,
    }
}

/// One step of the planned control API, applied locally.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "method")]
#[expect(
    variant_size_differences,
    reason = "camera.set_pose stores a pose; frames.wait and stats.query are markers"
)]
pub enum ScriptStep {
    /// Set the world-space camera. Applied by overwriting the scene pose.
    #[serde(rename = "camera.set_pose")]
    CameraSetPose {
        /// World position.
        position: [f32; 3],
        /// Rotation quaternion, xyzw.
        rotation_xyzw: [f32; 4],
    },
    /// Warmup frames, then measured frames. Counts come from the CLI.
    #[serde(rename = "frames.wait")]
    FramesWait {
        /// Frames discarded before measurement.
        warmup_frames: u32,
        /// Frames included in the percentiles.
        measured_frames: u32,
    },
    /// Read the benchmark report. The result is the JSON file, not a socket reply.
    #[serde(rename = "stats.query")]
    StatsQuery,
}

/// Script recorded for one scenario run.
pub fn script(scenario: &Scenario, warmup_frames: u32, measured_frames: u32) -> Vec<ScriptStep> {
    vec![
        ScriptStep::CameraSetPose {
            position: scenario.camera_position,
            rotation_xyzw: scenario.camera_rotation_xyzw,
        },
        ScriptStep::FramesWait {
            warmup_frames,
            measured_frames,
        },
        ScriptStep::StatsQuery,
    ]
}

/// Pose carried by the first `camera.set_pose` step.
pub fn pose_from_script(steps: &[ScriptStep]) -> Option<([f32; 3], [f32; 4])> {
    steps.iter().find_map(|step| match step {
        ScriptStep::CameraSetPose {
            position,
            rotation_xyzw,
        } => Some((*position, *rotation_xyzw)),
        ScriptStep::FramesWait { .. } | ScriptStep::StatsQuery => None,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_SCENARIOS, EXTRA_SCENARIOS, ScriptStep, by_name, pose_from_script, script,
    };

    fn method_names(steps: &[ScriptStep]) -> Vec<&'static str> {
        steps
            .iter()
            .map(|step| match step {
                ScriptStep::CameraSetPose { .. } => "camera.set_pose",
                ScriptStep::FramesWait { .. } => "frames.wait",
                ScriptStep::StatsQuery => "stats.query",
            })
            .collect()
    }

    #[test]
    fn default_scripts_use_the_control_api_method_names() {
        for name in DEFAULT_SCENARIOS {
            let scenario = by_name(name).expect(name);
            let steps = script(scenario, 8, 32);
            assert_eq!(
                method_names(&steps),
                ["camera.set_pose", "frames.wait", "stats.query"]
            );
            let (position, rotation) = pose_from_script(&steps).expect("pose");
            assert_eq!(position, scenario.camera_position);
            assert_eq!(rotation, scenario.camera_rotation_xyzw);
            let json = serde_json::to_string(&steps).expect("json");
            assert!(json.contains("\"method\":\"camera.set_pose\""), "{json}");
            assert!(json.contains("\"method\":\"frames.wait\""), "{json}");
            assert!(json.contains("\"method\":\"stats.query\""), "{json}");
        }
    }

    #[test]
    fn extra_scenarios_resolve_and_stay_out_of_the_default_set() {
        for name in EXTRA_SCENARIOS {
            assert!(by_name(name).is_some(), "{name} must resolve");
            assert!(!DEFAULT_SCENARIOS.contains(&name));
        }
    }

    #[test]
    fn many_shadowed_point_lights_is_opt_in() {
        assert!(by_name("many_shadowed_point_lights").is_some());
        assert!(!DEFAULT_SCENARIOS.contains(&"many_shadowed_point_lights"));
    }
}
