//! Sizing constants for the clustered-light compute pass.
//!
//! Binding 2 stores `[offset, count]` rows for each cluster; binding 3 stores the compact list
//! of light indices addressed by those rows. The constants below sit in the host so both the
//! compute pipeline setup and the frame BGL layout agree on the slab sizes.

/// Uniform buffer size for clustered light compute `ClusterParams`.
pub const CLUSTER_PARAMS_UNIFORM_SIZE: u64 = 256;

/// Number of `u32` words in one clustered-light range row.
pub const CLUSTER_LIGHT_RANGE_WORDS: u64 = 2;
