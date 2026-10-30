//! CPU-side uniform snapshots for the WARP `pbs_lit` canary.
//!
//! These lines record the camera, light, and material values the renderer
//! uploaded. They do not read those buffers back: the frame, light, and
//! material buffers are not `COPY_SRC`. A black frame with finite, non-zero
//! lights was not produced by an empty CPU upload.

use parking_lot::Mutex;

use super::frame_bindings::GpuLight;
use super::frame_globals::FrameGpuUniforms;
use super::warp_black_diag::frame_log_armed;

static LAST_FRAME: Mutex<String> = Mutex::new(String::new());
static LAST_LIGHTS: Mutex<String> = Mutex::new(String::new());
static SEEN_MATERIALS: Mutex<Vec<u64>> = Mutex::new(Vec::new());
static LAST_SKYBOX_CLEAR: Mutex<Option<[u32; 4]>> = Mutex::new(None);

const MAX_MATERIAL_LINES: usize = 32;

/// Logs camera, clip, light count, and ambient L0 when that tuple changes.
pub(crate) fn log_frame_uniforms(uniforms: &FrameGpuUniforms) {
    if !frame_log_armed() {
        return;
    }
    let line = format_frame_uniforms(uniforms);
    let changed = {
        let mut last = LAST_FRAME.lock();
        if *last == line {
            false
        } else {
            last.clone_from(&line);
            true
        }
    };
    if changed {
        logger::info!("{line}");
    }
}

/// Logs the packed light list when its summary changes.
pub(crate) fn log_lights(lights: &[GpuLight]) {
    if !frame_log_armed() {
        return;
    }
    let line = format_lights(lights);
    let changed = {
        let mut last = LAST_LIGHTS.lock();
        if *last == line {
            false
        } else {
            last.clone_from(&line);
            true
        }
    };
    if changed {
        logger::info!("{line}");
    }
}

/// Material uniform bytes just written into the arena.
pub(crate) struct MaterialBytes<'a> {
    /// Embedded shader stem.
    pub stem: &'a str,
    /// Packed uniform block.
    pub bytes: &'a [u8],
    /// Dynamic offset used when binding this slot.
    pub dynamic_offset: u32,
    /// Variant bits the packer was given.
    pub variant_bits: Option<u32>,
    /// Byte offset of `_Color`, when the block has that field.
    pub color_offset: Option<u32>,
    /// Byte offset of `_Metallic`.
    pub metallic_offset: Option<u32>,
    /// Byte offset of `_Glossiness`.
    pub gloss_offset: Option<u32>,
    /// Byte offset of `_RenderideVariantBits`.
    pub variant_offset: Option<u32>,
}

/// Logs one material upload the first time its summary is seen.
pub(crate) fn log_material_bytes(note: &MaterialBytes<'_>) {
    if !frame_log_armed() {
        return;
    }
    let line = format_material(note);
    let hash = fnv1a64(line.as_bytes());
    let fresh = {
        let mut seen = SEEN_MATERIALS.lock();
        if seen.contains(&hash) || seen.len() >= MAX_MATERIAL_LINES {
            false
        } else {
            seen.push(hash);
            true
        }
    };
    if fresh {
        logger::info!("{line}");
    }
}

/// Logs the skybox clear-color uniform when it changes.
pub(crate) fn log_skybox_clear(color: [f32; 4]) {
    if !frame_log_armed() {
        return;
    }
    let bits = [
        color[0].to_bits(),
        color[1].to_bits(),
        color[2].to_bits(),
        color[3].to_bits(),
    ];
    let changed = {
        let mut last = LAST_SKYBOX_CLEAR.lock();
        if *last == Some(bits) {
            false
        } else {
            *last = Some(bits);
            true
        }
    };
    if changed {
        logger::info!(
            "diag-pbs-skybox-clear: rgba={},{},{},{} non_finite={}",
            fmt_f32(color[0]),
            fmt_f32(color[1]),
            fmt_f32(color[2]),
            fmt_f32(color[3]),
            color.iter().any(|channel| !channel.is_finite()),
        );
    }
}

fn format_frame_uniforms(uniforms: &FrameGpuUniforms) -> String {
    let cam = uniforms.camera_world_pos;
    let ambient = uniforms.ambient_sh[0];
    format!(
        "diag-pbs-uniforms: cam={},{},{} near={} far={} lights={} viewport={}x{} proj_finite={} proj00={} non_finite={} ambient0={},{},{}",
        fmt_f32(cam[0]),
        fmt_f32(cam[1]),
        fmt_f32(cam[2]),
        fmt_f32(uniforms.near_clip),
        fmt_f32(uniforms.far_clip),
        uniforms.light_count,
        uniforms.viewport_width,
        uniforms.viewport_height,
        proj_finite(uniforms),
        fmt_f32(uniforms.proj_left[0]),
        frame_non_finite(uniforms),
        fmt_f32(ambient[0]),
        fmt_f32(ambient[1]),
        fmt_f32(ambient[2]),
    )
}

fn format_lights(lights: &[GpuLight]) -> String {
    let mut non_finite = 0u32;
    let mut zero_color = 0u32;
    let mut zero_intensity = 0u32;
    for light in lights {
        if light_non_finite(light) {
            non_finite = non_finite.saturating_add(1);
        }
        if light.color == [0.0, 0.0, 0.0] {
            zero_color = zero_color.saturating_add(1);
        }
        if light.intensity == 0.0 {
            zero_intensity = zero_intensity.saturating_add(1);
        }
    }
    let Some(first) = lights.first() else {
        return format!(
            "diag-pbs-lights: count=0 non_finite={non_finite} zero_color={zero_color} zero_intensity={zero_intensity} first=none"
        );
    };
    format!(
        "diag-pbs-lights: count={} non_finite={non_finite} zero_color={zero_color} zero_intensity={zero_intensity} first_type={} first_color={},{},{} first_intensity={} first_pos={},{},{} first_range={}",
        lights.len(),
        first.light_type,
        fmt_f32(first.color[0]),
        fmt_f32(first.color[1]),
        fmt_f32(first.color[2]),
        fmt_f32(first.intensity),
        fmt_f32(first.position[0]),
        fmt_f32(first.position[1]),
        fmt_f32(first.position[2]),
        fmt_f32(first.range),
    )
}

fn format_material(note: &MaterialBytes<'_>) -> String {
    let color = note
        .color_offset
        .and_then(|offset| read_vec4(note.bytes, offset));
    let metallic = note
        .metallic_offset
        .and_then(|offset| read_f32(note.bytes, offset));
    let gloss = note
        .gloss_offset
        .and_then(|offset| read_f32(note.bytes, offset));
    let variant_buf = note
        .variant_offset
        .and_then(|offset| read_u32(note.bytes, offset));
    format!(
        "diag-pbs-material: stem={} offset={} bytes={} variant_bits={} variant_buf={} color={} metallic={} gloss={} non_finite={}",
        note.stem,
        note.dynamic_offset,
        note.bytes.len(),
        format_opt_u32(note.variant_bits),
        format_opt_u32(variant_buf),
        format_opt_vec4(color),
        format_opt_f32(metallic),
        format_opt_f32(gloss),
        non_finite_f32s(note.bytes),
    )
}

fn proj_finite(uniforms: &FrameGpuUniforms) -> bool {
    uniforms.near_clip.is_finite()
        && uniforms.far_clip.is_finite()
        && finite4(uniforms.proj_params_left)
        && finite16(&uniforms.proj_left)
}

fn frame_non_finite(uniforms: &FrameGpuUniforms) -> bool {
    !finite4(uniforms.camera_world_pos)
        || !proj_finite(uniforms)
        || uniforms.ambient_sh.iter().any(|row| !finite4(*row))
        || !finite4(uniforms.view_space_z_coeffs)
        || !finite4(uniforms.view_space_x_coeffs)
        || !finite4(uniforms.view_space_y_coeffs)
}

fn light_non_finite(light: &GpuLight) -> bool {
    !finite3(light.position)
        || !finite3(light.direction)
        || !finite3(light.color)
        || !light.intensity.is_finite()
        || !light.range.is_finite()
}

fn finite3(values: [f32; 3]) -> bool {
    values.iter().all(|value| value.is_finite())
}

fn finite4(values: [f32; 4]) -> bool {
    values.iter().all(|value| value.is_finite())
}

fn finite16(values: &[f32; 16]) -> bool {
    values.iter().all(|value| value.is_finite())
}

fn read_f32(bytes: &[u8], offset: u32) -> Option<f32> {
    let start = usize::try_from(offset).ok()?;
    let chunk = bytes.get(start..start.saturating_add(4))?;
    let arr: [u8; 4] = chunk.try_into().ok()?;
    Some(f32::from_le_bytes(arr))
}

fn read_u32(bytes: &[u8], offset: u32) -> Option<u32> {
    let start = usize::try_from(offset).ok()?;
    let chunk = bytes.get(start..start.saturating_add(4))?;
    let arr: [u8; 4] = chunk.try_into().ok()?;
    Some(u32::from_le_bytes(arr))
}

fn read_vec4(bytes: &[u8], offset: u32) -> Option<[f32; 4]> {
    Some([
        read_f32(bytes, offset)?,
        read_f32(bytes, offset.saturating_add(4))?,
        read_f32(bytes, offset.saturating_add(8))?,
        read_f32(bytes, offset.saturating_add(12))?,
    ])
}

fn non_finite_f32s(bytes: &[u8]) -> u32 {
    let mut count = 0u32;
    let (chunks, _) = bytes.as_chunks::<4>();
    for chunk in chunks {
        if !f32::from_le_bytes(*chunk).is_finite() {
            count = count.saturating_add(1);
        }
    }
    count
}

fn format_opt_u32(value: Option<u32>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "missing".to_owned(),
    }
}

fn format_opt_f32(value: Option<f32>) -> String {
    match value {
        Some(value) => fmt_f32(value),
        None => "missing".to_owned(),
    }
}

fn format_opt_vec4(value: Option<[f32; 4]>) -> String {
    match value {
        Some(value) => format!(
            "{},{},{},{}",
            fmt_f32(value[0]),
            fmt_f32(value[1]),
            fmt_f32(value[2]),
            fmt_f32(value[3])
        ),
        None => "missing".to_owned(),
    }
}

fn fmt_f32(value: f32) -> String {
    format!("{value:.4}")
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use bytemuck::Zeroable;

    use super::{
        MaterialBytes, finite4, format_frame_uniforms, format_lights, format_material,
        frame_non_finite, light_non_finite, non_finite_f32s, proj_finite,
    };
    use crate::gpu::frame_bindings::GpuLight;
    use crate::gpu::frame_globals::FrameGpuUniforms;

    #[test]
    fn material_summary_reads_color_metallic_and_nan() {
        let mut bytes = vec![0u8; 112];
        bytes[0..16].copy_from_slice(&vec4_bytes([1.0, 0.25, 0.0, 1.0]));
        bytes[68..72].copy_from_slice(&1.0f32.to_le_bytes());
        bytes[84..88].copy_from_slice(&0.5f32.to_le_bytes());
        bytes[104..108].copy_from_slice(&0u32.to_le_bytes());
        bytes[108..112].copy_from_slice(&f32::NAN.to_le_bytes());
        let line = format_material(&MaterialBytes {
            stem: "pbsmetallic_default",
            bytes: &bytes,
            dynamic_offset: 256,
            variant_bits: Some(0),
            color_offset: Some(0),
            metallic_offset: Some(84),
            gloss_offset: Some(68),
            variant_offset: Some(104),
        });
        assert!(line.contains("stem=pbsmetallic_default"), "{line}");
        assert!(line.contains("offset=256"), "{line}");
        assert!(line.contains("color=1.0000,0.2500,0.0000,1.0000"), "{line}");
        assert!(line.contains("metallic=0.5000"), "{line}");
        assert!(line.contains("gloss=1.0000"), "{line}");
        assert!(line.contains("variant_bits=0"), "{line}");
        assert!(line.contains("variant_buf=0"), "{line}");
        assert!(line.contains("non_finite=1"), "{line}");
        assert_eq!(non_finite_f32s(&bytes), 1);
    }

    #[test]
    fn zero_light_list_is_explicit() {
        let line = format_lights(&[]);
        assert!(line.contains("count=0"), "{line}");
        assert!(line.contains("first=none"), "{line}");
    }

    #[test]
    fn light_summary_flags_zero_color_and_nan_intensity() {
        let light = GpuLight {
            color: [0.0, 0.0, 0.0],
            intensity: f32::NAN,
            light_type: 2,
            range: 4.0,
            position: [1.0, 2.0, 3.0],
            ..GpuLight::default()
        };
        assert!(light_non_finite(&light));
        let line = format_lights(&[light]);
        assert!(line.contains("count=1"), "{line}");
        assert!(line.contains("non_finite=1"), "{line}");
        assert!(line.contains("zero_color=1"), "{line}");
        assert!(line.contains("first_type=2"), "{line}");
        assert!(line.contains("first_pos=1.0000,2.0000,3.0000"), "{line}");
    }

    #[test]
    fn zeroed_frame_uniforms_are_finite_and_have_no_lights() {
        let uniforms = FrameGpuUniforms::zeroed();
        assert!(proj_finite(&uniforms));
        assert!(!frame_non_finite(&uniforms));
        assert!(finite4(uniforms.camera_world_pos));
        let line = format_frame_uniforms(&uniforms);
        assert!(line.contains("lights=0"), "{line}");
        assert!(line.contains("proj00=0.0000"), "{line}");
        assert!(line.contains("non_finite=false"), "{line}");
    }

    fn vec4_bytes(values: [f32; 4]) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        for (index, value) in values.into_iter().enumerate() {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }
}
