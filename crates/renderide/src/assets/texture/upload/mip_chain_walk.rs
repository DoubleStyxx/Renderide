//! Shared per-mip payload-offset resolution shared by 2D, cubemap, and subregion uploads.
//!
//! Each upload path counts how many mips of a chain fit inside the SHM descriptor window after
//! applying a `start_bias` (the descriptor offset). The arithmetic and stop conditions are the
//! same; only the per-mip key (face index, mip index) and error wording differ. This module
//! exposes one resolver function plus a stop-reason enum so each caller stays in charge of
//! emitting its own diagnostics.

use crate::shared::TextureFormat;

use super::super::layout::host_mip_payload_byte_offset;
use super::error::TextureUploadError;

/// Reason a mip cannot be uploaded; the caller decides whether to break, warn, or fail.
#[derive(Debug)]
pub(super) enum MipChainStop {
    /// Host wrote a negative `mip_starts` entry.
    NegativeStart,
    /// Host wrote a `mip_starts` entry inside the descriptor bias window.
    BeforeBias,
    /// The mip's byte range extends past `payload_len`.
    OutOfPayload,
}

/// Validates one mip's host byte range against `payload_len`, rebasing by descriptor bias.
///
/// `host_len` is the result of [`mip_byte_len`](super::super::layout::mip_byte_len) and is taken
/// as input so each caller can preserve its own byte-size error wording. `offset_label` is invoked
/// only when [`host_mip_payload_byte_offset`] cannot convert the rebased mip start.
///
/// Returns:
/// - `Ok(Ok(()))` if the mip fits inside the payload.
/// - `Ok(Err(stop))` if the chain should stop walking here.
/// - `Err(_)` if the format produces an unsupported byte offset.
pub(super) fn resolve_mip_payload_slot(
    format: TextureFormat,
    host_len: usize,
    start_raw: i32,
    bias: usize,
    payload_len: usize,
    offset_label: impl FnOnce() -> String,
) -> Result<Result<(), MipChainStop>, TextureUploadError> {
    if start_raw < 0 {
        return Ok(Err(MipChainStop::NegativeStart));
    }
    let start_abs = start_raw as usize;
    if start_abs < bias {
        return Ok(Err(MipChainStop::BeforeBias));
    }
    let start_rel = start_abs - bias;
    let start = host_mip_payload_byte_offset(format, start_rel).ok_or_else(|| {
        TextureUploadError::from(format!(
            "{}: could not convert mip_starts offset to bytes for {format:?}",
            offset_label()
        ))
    })?;
    if start
        .checked_add(host_len)
        .is_none_or(|end| end > payload_len)
    {
        return Ok(Err(MipChainStop::OutOfPayload));
    }
    Ok(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_start_stops() {
        let result = resolve_mip_payload_slot(TextureFormat::RGBA32, 64, -1, 0, 1024, String::new)
            .expect("no hard error");
        assert!(matches!(result, Err(MipChainStop::NegativeStart)));
    }

    #[test]
    fn before_bias_stops() {
        let result =
            resolve_mip_payload_slot(TextureFormat::RGBA32, 64, 100, 200, 1024, String::new)
                .expect("no hard error");
        assert!(matches!(result, Err(MipChainStop::BeforeBias)));
    }

    #[test]
    fn out_of_payload_stops() {
        let result =
            resolve_mip_payload_slot(TextureFormat::RGBA32, 64, 1000, 0, 1024, String::new)
                .expect("no hard error");
        assert!(matches!(result, Err(MipChainStop::OutOfPayload)));
    }

    #[test]
    fn rebased_slot_accepts_in_bounds_mip() {
        let result =
            resolve_mip_payload_slot(TextureFormat::RGBA32, 64, 256, 64, 4096, String::new)
                .expect("no hard error");
        assert!(result.is_ok());
    }
}
