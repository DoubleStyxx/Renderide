//! JSON-RPC 2.0 envelopes and control-method payloads.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// JSON-RPC parse error.
pub const CODE_PARSE_ERROR: i64 = -32_700;
/// JSON-RPC invalid request.
pub const CODE_INVALID_REQUEST: i64 = -32_600;
/// JSON-RPC method not found.
pub const CODE_METHOD_NOT_FOUND: i64 = -32_601;
/// JSON-RPC invalid params.
pub const CODE_INVALID_PARAMS: i64 = -32_602;
/// JSON-RPC internal error.
pub const CODE_INTERNAL_ERROR: i64 = -32_603;

/// `scene.load` is recognized and rejected until a renderer-side loader exists.
pub const SCENE_LOAD_TODO: &str = "Method not found: scene.load. TODO: scene.load is not implemented; host IPC owns scene submission";

/// Wire name for `scene.load`.
pub const fn method_scene_load() -> &'static str {
    "scene.load"
}

/// Empty success object returned by commands that only mutate renderer state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ack {
    /// Always `true` when the command was accepted.
    pub ok: bool,
}

impl Ack {
    /// Builds the standard accepted response.
    pub const fn ok() -> Self {
        Self { ok: true }
    }
}

/// Absolute or relative pointer motion.
///
/// `x`/`y` set the window position when present. `dx`/`dy` are added to the
/// per-frame direct delta and, when `x` or `y` is omitted, to the stored position.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MouseMoveParams {
    /// Window x in logical pixels.
    #[serde(default)]
    pub x: Option<f32>,
    /// Window y in logical pixels.
    #[serde(default)]
    pub y: Option<f32>,
    /// Horizontal direct delta.
    #[serde(default)]
    pub dx: f32,
    /// Vertical direct delta.
    #[serde(default)]
    pub dy: f32,
}

/// Button press or release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MouseClickParams {
    /// `left`, `right`, `middle`, `button4`, or `button5`.
    pub button: String,
    /// Held state. Defaults to pressed.
    #[serde(default = "pressed_by_default")]
    pub pressed: bool,
}

fn pressed_by_default() -> bool {
    true
}

/// Scroll wheel delta in host wheel units.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MouseScrollParams {
    /// Horizontal wheel delta.
    #[serde(default)]
    pub dx: f32,
    /// Vertical wheel delta.
    #[serde(default)]
    pub dy: f32,
}

/// Host key name for `input.key_down` / `input.key_up`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyParams {
    /// Key label such as `W`, `Space`, `LeftShift`, or `Escape`.
    pub key: String,
}

/// Camera position, rotation, and vertical field of view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraPoseParams {
    /// World position `[x, y, z]`.
    pub pos: [f32; 3],
    /// Rotation quaternion `[x, y, z, w]`.
    pub rot: [f32; 4],
    /// Vertical field of view in degrees.
    pub fov: f32,
}

/// Keyframed camera playback. Keyframe `i` is shown at time `i * dt` seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayPathParams {
    /// Poses sampled along the path.
    pub keyframes: Vec<CameraPoseParams>,
    /// Seconds between adjacent keyframes.
    pub dt: f32,
}

/// `frames.wait` argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FramesWaitParams {
    /// Number of additional presented frames to wait for.
    pub n: u64,
}

/// Result of `frames.wait`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FramesWaitResult {
    /// Index of the presented frame that completed the wait.
    ///
    /// Frame indices start at 0. When no frame has been presented and `n` is 0,
    /// this is 0.
    pub frame_index: u64,
    /// Total presented frames observed by the server after the wait.
    pub presented: u64,
}

/// CPU phase times for one presented frame, in milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CpuPhaseMs {
    /// Draw queue and sort time.
    pub draw_prep: f64,
    /// View cull snapshot time.
    pub cull: f64,
    /// Upload drain time.
    pub upload: f64,
    /// Submit enqueue time.
    pub submit: f64,
}

/// One raw presented-frame sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameStats {
    /// Monotonic presented-frame index, starting at 0.
    pub frame_index: u64,
    /// Render-thread CPU time for the tick, in milliseconds.
    pub cpu_frame_ms: f64,
    /// Named CPU phase times.
    pub cpu_phase_ms: CpuPhaseMs,
    /// Per-pass GPU milliseconds. `null` when the device lacks `TIMESTAMP_QUERY`.
    pub gpu_pass_ms: Option<BTreeMap<String, f64>>,
    /// Indexed draws queued for the main view.
    pub draw_calls: u64,
    /// Instances represented by those draws.
    pub instances: u64,
    /// Upload writes drained with the frame.
    pub upload_count: u64,
    /// Upload payload bytes drained with the frame.
    pub upload_bytes: u64,
    /// Triangles in the queued main-view draws (`index_count / 3`).
    pub triangles: u64,
}

/// `stats.query` argument.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsQueryParams {
    /// Inclusive lower bound on [`FrameStats::frame_index`].
    #[serde(default)]
    pub since_frame: u64,
}

/// `capture.png` argument.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturePngParams {
    /// Destination PNG path.
    pub path: String,
}

/// `capture.png` result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturePngResult {
    /// Path that was written.
    pub path: String,
    /// Presented frame captured.
    pub frame_index: u64,
}

/// Renderer identity captured once the GPU device exists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaInfo {
    /// Adapter name.
    pub adapter: String,
    /// Driver name.
    pub driver: String,
    /// Graphics backend.
    pub backend: String,
    /// Active present mode.
    pub present_mode: String,
    /// Framebuffer width and height in pixels.
    pub resolution: [u32; 2],
    /// Embedded git commit, or `unavailable`.
    pub git_sha: String,
}

/// One decoded request line.
#[derive(Clone, Debug)]
pub struct ParsedRequest {
    /// `None` when the request is a notification (no `id` member).
    pub id: Option<Value>,
    /// Method name.
    pub method: String,
    /// Params object. Missing or null params become `{}`.
    pub params: Value,
}

/// Why a request line was rejected, plus any id that could be recovered.
#[derive(Clone, Debug)]
pub struct RequestParseFailure {
    /// Id to echo, when the payload was JSON and contained one.
    pub id: Option<Value>,
    /// JSON-RPC error code.
    pub code: i64,
    /// Human-readable reason.
    pub message: String,
}

/// Error object carried by a response line.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RpcErrorObject {
    /// JSON-RPC error code.
    pub code: i64,
    /// Error text.
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct RpcSuccessEnvelope {
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<RpcErrorObject>,
    id: Option<Value>,
}

/// Parses one newline-delimited JSON-RPC request.
pub fn parse_request_line(line: &str) -> Result<ParsedRequest, RequestParseFailure> {
    let value: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(err) => {
            return Err(RequestParseFailure {
                id: None,
                code: CODE_PARSE_ERROR,
                message: err.to_string(),
            });
        }
    };
    let id = value.get("id").cloned();
    if !value.is_object() {
        return Err(RequestParseFailure {
            id,
            code: CODE_INVALID_REQUEST,
            message: "request must be a JSON object".to_owned(),
        });
    }
    if value.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(RequestParseFailure {
            id,
            code: CODE_INVALID_REQUEST,
            message: "jsonrpc must be \"2.0\"".to_owned(),
        });
    }
    let Some(method) = value.get("method").and_then(Value::as_str) else {
        return Err(RequestParseFailure {
            id,
            code: CODE_INVALID_REQUEST,
            message: "method must be a string".to_owned(),
        });
    };
    let params = match value.get("params") {
        None | Some(Value::Null) => Value::Object(Map::new()),
        Some(params) => params.clone(),
    };
    Ok(ParsedRequest {
        id,
        method: method.to_owned(),
        params,
    })
}

/// Deserializes method params.
pub fn from_params<T: DeserializeOwned>(params: &Value) -> Result<T, String> {
    serde_json::from_value(params.clone()).map_err(|err| err.to_string())
}

/// Encodes a success response, including the trailing newline.
pub fn success_line(id: &Value, result: &impl Serialize) -> Result<String, serde_json::Error> {
    let result = serde_json::to_value(result)?;
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    });
    Ok(finish_line(body))
}

/// Encodes an error response, including the trailing newline.
pub fn error_line(id: &Value, code: i64, message: &str) -> String {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    });
    finish_line(body)
}

fn finish_line(body: Value) -> String {
    let mut line = body.to_string();
    line.push('\n');
    line
}

/// One decoded response line.
pub(crate) struct DecodedResponse {
    /// Id echoed by the server.
    pub id: Option<Value>,
    /// Result value, or the error object.
    pub body: Result<Value, RpcErrorObject>,
}

/// Decodes one response line.
pub(crate) fn parse_response_line(line: &str) -> Result<DecodedResponse, String> {
    let envelope: RpcSuccessEnvelope = serde_json::from_str(line).map_err(|err| err.to_string())?;
    let body = if let Some(error) = envelope.error {
        Err(error)
    } else {
        Ok(envelope.result.unwrap_or(Value::Null))
    };
    Ok(DecodedResponse {
        id: envelope.id,
        body,
    })
}

/// Encodes a request line, including the trailing newline.
pub(crate) fn request_line(
    id: u64,
    method: &str,
    params: &impl Serialize,
) -> Result<String, serde_json::Error> {
    let params = serde_json::to_value(params)?;
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    Ok(finish_line(body))
}

#[cfg(test)]
mod tests {
    use super::{
        CODE_PARSE_ERROR, FrameStats, SCENE_LOAD_TODO, error_line, parse_request_line, success_line,
    };
    use serde_json::Value;

    #[test]
    fn request_line_keeps_id_and_method() {
        let parsed = parse_request_line(
            r#"{"jsonrpc":"2.0","id":7,"method":"frames.wait","params":{"n":3}}"#,
        )
        .expect("parse");
        assert_eq!(parsed.id, Some(Value::from(7)));
        assert_eq!(parsed.method, "frames.wait");
        assert_eq!(parsed.params["n"], 3);
    }

    #[test]
    fn missing_id_is_a_notification() {
        let parsed = parse_request_line(r#"{"jsonrpc":"2.0","method":"meta"}"#).expect("parse");
        assert!(parsed.id.is_none());
        assert!(parsed.params.is_object());
    }

    #[test]
    fn malformed_json_is_a_parse_error_without_id() {
        let err = parse_request_line("not-json").expect_err("bad");
        assert_eq!(err.code, CODE_PARSE_ERROR);
        assert!(err.id.is_none());
    }

    #[test]
    fn frame_stats_serialize_gpu_passes_as_null() {
        let stats = FrameStats {
            frame_index: 0,
            cpu_frame_ms: 1.5,
            cpu_phase_ms: super::CpuPhaseMs::default(),
            gpu_pass_ms: None,
            draw_calls: 2,
            instances: 2,
            upload_count: 1,
            upload_bytes: 64,
            triangles: 8,
        };
        let value = serde_json::to_value(&stats).expect("json");
        assert!(value["gpu_pass_ms"].is_null());
        assert_eq!(value["triangles"], 8);
    }

    #[test]
    fn error_line_includes_scene_load_todo() {
        let line = error_line(
            &Value::from(1),
            super::CODE_METHOD_NOT_FOUND,
            SCENE_LOAD_TODO,
        );
        assert!(line.contains("TODO"));
        assert!(line.ends_with('\n'));
        let _ = success_line(&Value::from(1), &super::Ack::ok()).expect("success");
    }
}
