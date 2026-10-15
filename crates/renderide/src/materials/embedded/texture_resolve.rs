//! Texture asset id resolution and bind signature hashing for embedded material bind groups.

mod bind_signature;
mod lookup;
mod sampler;

pub(crate) use bind_signature::{
    hash_texture_entry_signature_contribution, texture_bind_signature,
};
pub(crate) use lookup::{
    DefaultTextureColor, ResolvedTextureBinding, default_2d_texture_color_for_host,
    primary_texture_2d_asset_id, resolved_texture_binding_for_host,
    texture_property_ids_for_binding,
};
pub(crate) use sampler::{create_sampler, default_embedded_sampler};
