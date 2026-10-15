//! Bounds-checked random-access reader for one attribute of an interleaved vertex buffer.
//!
//! Centralizes the `(vertex * stride) + offset` arithmetic, overflow-checked bound validation, and
//! attribute lookup that mesh stream extraction and tangent generation both perform. Construction
//! returns `None` if the attribute is missing, has fewer than the requested number of components,
//! or the resulting byte range would extend past the end of `vertex_data`.

use crate::shared::{VertexAttributeDescriptor, VertexAttributeType};

use super::super::layout::{
    VertexDecodeKind, attribute_offset_and_size, decode_vertex_vec2, decode_vertex_vec3,
    decode_vertex_vec4,
};

/// Bounds-validated view over one attribute of an interleaved vertex buffer.
pub(in crate::assets::mesh) struct AttributeReader<'a> {
    vertex_data: &'a [u8],
    stride: usize,
    offset: usize,
    attr: VertexAttributeDescriptor,
    kind: VertexDecodeKind,
}

impl<'a> AttributeReader<'a> {
    /// Constructs a reader for the first attribute of `target` type with at least `min_dimensions`
    /// components, after validating that the resulting byte range fits in `vertex_data`.
    pub(in crate::assets::mesh) fn from_attrs(
        vertex_data: &'a [u8],
        vertex_count: usize,
        stride: usize,
        attrs: &[VertexAttributeDescriptor],
        target: VertexAttributeType,
        kind: VertexDecodeKind,
        min_dimensions: u32,
    ) -> Option<Self> {
        if vertex_count == 0 || stride == 0 {
            return None;
        }
        let need = vertex_count.checked_mul(stride)?;
        if vertex_data.len() < need {
            return None;
        }
        let attr = attrs
            .iter()
            .copied()
            .find(|a| (a.attribute as i16) == (target as i16))?;
        if (attr.dimensions as u32) < min_dimensions {
            return None;
        }
        let (offset, size) = attribute_offset_and_size(attrs, target)?;
        let last_base = (vertex_count - 1)
            .checked_mul(stride)?
            .checked_add(offset)?;
        if last_base.checked_add(size)? > vertex_data.len() {
            return None;
        }
        Some(Self {
            vertex_data,
            stride,
            offset,
            attr,
            kind,
        })
    }

    /// Reads a two-component float vector at the given vertex index.
    pub(in crate::assets::mesh) fn read_vec2(&self, vertex: usize) -> Option<[f32; 2]> {
        decode_vertex_vec2(self.vertex_data, self.base(vertex), self.attr, self.kind)
    }

    /// Reads a three-component float vector at the given vertex index.
    pub(in crate::assets::mesh) fn read_vec3(&self, vertex: usize) -> Option<[f32; 3]> {
        decode_vertex_vec3(self.vertex_data, self.base(vertex), self.attr, self.kind)
    }

    /// Reads up to four float components at the given vertex index, defaulting missing components
    /// from `default`.
    pub(in crate::assets::mesh) fn read_vec4(
        &self,
        vertex: usize,
        default: [f32; 4],
    ) -> Option<[f32; 4]> {
        decode_vertex_vec4(
            self.vertex_data,
            self.base(vertex),
            self.attr,
            self.kind,
            default,
        )
    }

    fn base(&self, vertex: usize) -> usize {
        vertex * self.stride + self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::{VertexAttributeFormat, VertexAttributeType};

    fn float3_attr(target: VertexAttributeType) -> VertexAttributeDescriptor {
        VertexAttributeDescriptor {
            attribute: target,
            format: VertexAttributeFormat::Float32,
            dimensions: 3,
        }
    }

    #[test]
    fn returns_none_when_vertex_count_zero() {
        let attrs = [float3_attr(VertexAttributeType::Position)];
        let reader = AttributeReader::from_attrs(
            &[],
            0,
            12,
            &attrs,
            VertexAttributeType::Position,
            VertexDecodeKind::Position,
            3,
        );
        assert!(reader.is_none());
    }

    #[test]
    fn returns_none_when_stride_zero() {
        let attrs = [float3_attr(VertexAttributeType::Position)];
        let reader = AttributeReader::from_attrs(
            &[0u8; 64],
            4,
            0,
            &attrs,
            VertexAttributeType::Position,
            VertexDecodeKind::Position,
            3,
        );
        assert!(reader.is_none());
    }

    #[test]
    fn returns_none_when_attribute_missing() {
        let attrs = [float3_attr(VertexAttributeType::Position)];
        let reader = AttributeReader::from_attrs(
            &[0u8; 64],
            4,
            12,
            &attrs,
            VertexAttributeType::Tangent,
            VertexDecodeKind::Direction,
            3,
        );
        assert!(reader.is_none());
    }

    #[test]
    fn returns_none_when_dimensions_below_minimum() {
        let mut attr = float3_attr(VertexAttributeType::UV0);
        attr.dimensions = 1;
        let attrs = [attr];
        let reader = AttributeReader::from_attrs(
            &[0u8; 64],
            4,
            4,
            &attrs,
            VertexAttributeType::UV0,
            VertexDecodeKind::TexCoord,
            2,
        );
        assert!(reader.is_none());
    }

    #[test]
    fn returns_none_when_buffer_too_short() {
        let attrs = [float3_attr(VertexAttributeType::Position)];
        let reader = AttributeReader::from_attrs(
            &[0u8; 8],
            4,
            12,
            &attrs,
            VertexAttributeType::Position,
            VertexDecodeKind::Position,
            3,
        );
        assert!(reader.is_none());
    }

    #[test]
    fn returns_none_when_vertex_count_overflows_stride() {
        let attrs = [float3_attr(VertexAttributeType::Position)];
        let reader = AttributeReader::from_attrs(
            &[0u8; 64],
            usize::MAX,
            usize::MAX,
            &attrs,
            VertexAttributeType::Position,
            VertexDecodeKind::Position,
            3,
        );
        assert!(reader.is_none());
    }

    #[test]
    fn reads_float3_position() {
        let attrs = [float3_attr(VertexAttributeType::Position)];
        let mut data = Vec::new();
        for vertex in 0..3 {
            for component in 0..3 {
                let value = (vertex * 3 + component) as f32;
                data.extend_from_slice(&value.to_le_bytes());
            }
        }
        let reader = AttributeReader::from_attrs(
            &data,
            3,
            12,
            &attrs,
            VertexAttributeType::Position,
            VertexDecodeKind::Position,
            3,
        )
        .expect("reader should construct");
        assert_eq!(reader.read_vec3(0), Some([0.0, 1.0, 2.0]));
        assert_eq!(reader.read_vec3(1), Some([3.0, 4.0, 5.0]));
        assert_eq!(reader.read_vec3(2), Some([6.0, 7.0, 8.0]));
    }
}
