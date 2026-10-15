//! No-op implementation of the resource-churn counters when the `tracy` feature is off.

use super::ResourceChurnKind;

/// Static counter state for one resource creation site.
#[derive(Debug)]
pub(crate) struct ResourceChurnSite;

impl ResourceChurnSite {
    /// Creates a no-op resource-churn site when Tracy is disabled.
    pub(crate) const fn new(_kind: ResourceChurnKind, _label: &'static str) -> Self {
        Self
    }

    /// Records one resource creation at this site.
    #[inline]
    pub(crate) fn note(&'static self) {}
}

/// No-op frame flush when Tracy is disabled.
#[inline]
pub(crate) fn flush_resource_churn_plots() {}
