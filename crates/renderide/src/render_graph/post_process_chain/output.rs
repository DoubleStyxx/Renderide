//! Output handle and pass-range bookkeeping returned by `PostProcessChain::build_into_graph`.

use crate::render_graph::ids::PassId;
use crate::render_graph::resources::TextureHandle;

/// Result of [`super::chain::PostProcessChain::build_into_graph`].
#[derive(Clone, Copy, Debug)]
#[expect(
    variant_size_differences,
    reason = "Copy enum; `Chained` carries pass-id range inline to avoid heap for a one-shot result"
)]
pub enum ChainOutput {
    /// No effects ran; the chain forwards the original input handle.
    PassThrough(TextureHandle),
    /// One or more effects ran; the chain output and pass-id range are returned so the caller
    /// can wire explicit edges.
    Chained {
        /// Final HDR output of the chain.
        final_handle: TextureHandle,
        /// First pass added by the chain.
        first_pass: PassId,
        /// Last pass added by the chain.
        last_pass: PassId,
    },
}

impl ChainOutput {
    /// Returns the final HDR handle the next consumer should read.
    pub fn final_handle(self) -> TextureHandle {
        match self {
            Self::PassThrough(h) => h,
            Self::Chained { final_handle, .. } => final_handle,
        }
    }

    /// Returns the first/last pass ids when the chain produced any pass.
    pub fn pass_range(self) -> Option<(PassId, PassId)> {
        match self {
            Self::PassThrough(_) => None,
            Self::Chained {
                first_pass,
                last_pass,
                ..
            } => Some((first_pass, last_pass)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_output_helpers() {
        let h = TextureHandle(7);
        let pt = ChainOutput::PassThrough(h);
        assert_eq!(pt.final_handle(), h);
        assert!(pt.pass_range().is_none());

        let chained = ChainOutput::Chained {
            final_handle: h,
            first_pass: PassId(1),
            last_pass: PassId(2),
        };
        assert_eq!(chained.final_handle(), h);
        assert_eq!(chained.pass_range(), Some((PassId(1), PassId(2))));
    }
}
