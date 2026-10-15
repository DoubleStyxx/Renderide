//! Integration: roundtrip tests for [`MemoryPacker`] and [`MemoryUnpacker`] wire primitives
//! (host-compatible layout).

use renderide_shared::WireDecodeError;
use renderide_shared::default_entity_pool::DefaultEntityPool;
use renderide_shared::enum_repr::EnumRepr;
use renderide_shared::memory_packable::MemoryPackable;
use renderide_shared::memory_packer::MemoryPacker;
use renderide_shared::memory_unpack_error::MemoryUnpackError;
use renderide_shared::memory_unpacker::{MAX_STRING_LEN, MemoryUnpacker};
use renderide_shared::packed_bools::PackedBools;
use renderide_shared::packing::memory_pack_error::MemoryPackError;

#[test]
fn pack_unpack_bool_roundtrip() {
    let mut buf = [0u8; 8];
    let full = buf.len();
    {
        let mut p = MemoryPacker::new(&mut buf);
        p.write_bool(true);
        p.write_bool(false);
        assert_eq!(full - p.remaining_len(), 2);
    };
    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf[..2], &mut pool);
    assert!(u.read_bool().expect("bool"));
    assert!(!u.read_bool().expect("bool"));
}

#[test]
fn pack_unpack_i32_and_u8_roundtrip() {
    let mut buf = [0u8; 16];
    {
        let mut p = MemoryPacker::new(&mut buf);
        p.write(&42i32);
        p.write(&0xabu8);
    };
    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf, &mut pool);
    assert_eq!(u.read::<i32>().expect("i32"), 42);
    assert_eq!(u.read::<u8>().expect("u8"), 0xab);
}

#[test]
fn pack_unpack_option_roundtrip() {
    let mut buf = [0u8; 32];
    {
        let mut p = MemoryPacker::new(&mut buf);
        p.write_option::<i32>(None);
        p.write_option(Some(&7i32));
    };
    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf, &mut pool);
    assert_eq!(u.read_option::<i32>().expect("opt"), None);
    assert_eq!(u.read_option::<i32>().expect("opt"), Some(7));
}

#[test]
fn pack_unpack_str_none_empty_ascii_and_utf16() {
    const CAP: usize = 256;
    let mut buf = [0u8; CAP];
    let hi = "こんにちは";
    let written = {
        let mut p = MemoryPacker::new(&mut buf);
        p.write_str(None);
        p.write_str(Some(""));
        p.write_str(Some("ascii"));
        p.write_str(Some(hi));
        CAP - p.remaining_len()
    };
    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf[..written], &mut pool);
    assert_eq!(u.read_str().expect("s"), None);
    assert_eq!(u.read_str().expect("s").as_deref(), Some(""));
    assert_eq!(u.read_str().expect("s").as_deref(), Some("ascii"));
    assert_eq!(u.read_str().expect("s").as_deref(), Some(hi));
}

#[test]
fn read_truncated_mid_i32_returns_underrun() {
    let buf = [0x2a, 0x00];
    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf, &mut pool);
    let err = u.read::<i32>().unwrap_err();
    assert_eq!(err, MemoryUnpackError::pod_underrun::<i32>(4, 2));
}

#[test]
fn access_count_times_elem_size_overflow_returns_length_overflow() {
    let buf = [0u8; 4];
    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf, &mut pool);
    let err = u.access::<u16>(usize::MAX).unwrap_err();
    assert_eq!(err, MemoryUnpackError::LengthOverflow);
}

#[test]
fn packed_bools_from_byte_matches_bit_positions() {
    assert_eq!(
        PackedBools::from_byte(0x00),
        PackedBools {
            bit0: false,
            bit1: false,
            bit2: false,
            bit3: false,
            bit4: false,
            bit5: false,
            bit6: false,
            bit7: false,
        }
    );
    assert_eq!(
        PackedBools::from_byte(0xFF).eight(),
        (true, true, true, true, true, true, true, true)
    );
    assert_eq!(
        PackedBools::from_byte(0xAA).eight(),
        (false, true, false, true, false, true, false, true)
    );
}

#[test]
fn write_packed_bools_array_roundtrips_through_unpacker() {
    let mut buf = [0u8; 4];
    let pattern = [true, false, true, false, true, false, true, false];
    {
        let mut p = MemoryPacker::new(&mut buf);
        p.write_packed_bools_array(pattern);
    };
    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf[..1], &mut pool);
    let got = u.read_packed_bools().expect("packed");
    assert_eq!(
        got.eight(),
        (true, false, true, false, true, false, true, false)
    );
    assert_eq!(PackedBools::from_byte(buf[0]).eight(), got.eight());
}

#[test]
fn pack_unpack_value_list_i32_roundtrip_empty_one_and_many() {
    for items in [vec![], vec![42i32], vec![1, 2, 3, -7]] {
        let mut buf = vec![0u8; 4 + items.len() * 4];
        let cap = buf.len();
        let written = {
            let mut p = MemoryPacker::new(&mut buf);
            p.write_value_list(Some(items.as_slice()));
            cap - p.remaining_len()
        };
        let mut pool = DefaultEntityPool;
        let mut u = MemoryUnpacker::new(&buf[..written], &mut pool);
        let got: Vec<i32> = u.read_value_list().expect("decoded");
        assert_eq!(got, items);
    }
}

#[test]
fn pack_unpack_value_list_none_writes_zero_count() {
    let mut buf = [0u8; 8];
    {
        let mut p = MemoryPacker::new(&mut buf);
        p.write_value_list::<u8>(None);
    };
    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf[..4], &mut pool);
    let got: Vec<u8> = u.read_value_list().expect("ok");
    assert!(got.is_empty());
    assert_eq!(&buf[..4], &0i32.to_le_bytes());
}

#[test]
fn pack_unpack_string_list_with_mixed_none_and_some() {
    let mut buf = vec![0u8; 256];
    let cap = buf.len();
    let written = {
        let mut p = MemoryPacker::new(&mut buf);
        let entries: Vec<Option<&str>> = vec![None, Some("abc"), Some(""), Some("ßç")];
        p.write_string_list(Some(&entries));
        cap - p.remaining_len()
    };
    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf[..written], &mut pool);
    let got = u.read_string_list().expect("decoded");
    assert_eq!(got.len(), 4);
    assert_eq!(got[0], None);
    assert_eq!(got[1].as_deref(), Some("abc"));
    assert_eq!(got[2].as_deref(), Some(""));
    assert_eq!(got[3].as_deref(), Some("ßç"));
}

/// Local enum used to exercise [`MemoryPacker::write_enum_value_list`] without depending on a
/// generated host enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestKind {
    Zero,
    One,
    Negative,
    Large,
}

impl EnumRepr for TestKind {
    fn as_i32(self) -> i32 {
        match self {
            TestKind::Zero => 0,
            TestKind::One => 1,
            TestKind::Negative => -3,
            TestKind::Large => 4242,
        }
    }

    fn from_i32(i: i32) -> Self {
        match i {
            0 => TestKind::Zero,
            1 => TestKind::One,
            -3 => TestKind::Negative,
            _ => TestKind::Large,
        }
    }
}

#[test]
fn pack_unpack_enum_value_list_roundtrip() {
    let entries = [
        TestKind::Zero,
        TestKind::One,
        TestKind::Negative,
        TestKind::Large,
    ];
    let mut buf = vec![0u8; 4 + entries.len() * 4];
    let cap = buf.len();
    let written = {
        let mut p = MemoryPacker::new(&mut buf);
        p.write_enum_value_list(Some(&entries));
        cap - p.remaining_len()
    };
    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf[..written], &mut pool);
    let got: Vec<TestKind> = u.read_enum_value_list().expect("decoded");
    assert_eq!(got, entries);
}

#[test]
fn packer_overflow_captures_first_failed_write_and_preserves_cursor() {
    let mut buf = [0u8; 5];
    let err = {
        let full = buf;
        let mut p = MemoryPacker::new(&mut buf);
        p.write(&0x1122_3344u32);
        p.write(&0x5566u16);
        p.write(&0x77u8);

        assert!(p.had_overflow());
        assert_eq!(p.remaining_len(), 1);
        assert_eq!(
            p.overflow_error(),
            Some(MemoryPackError::BufferTooSmall {
                ty: "u16",
                needed: 2,
                remaining: 1,
            })
        );
        p.into_result(&full).unwrap_err()
    };

    assert_eq!(
        err,
        MemoryPackError::BufferTooSmall {
            ty: "u16",
            needed: 2,
            remaining: 1,
        }
    );
    assert_eq!(&buf[..4], &0x1122_3344u32.to_le_bytes());
    assert_eq!(buf[4], 0);
}

#[test]
fn pack_unpack_nested_value_list_roundtrip() {
    let nested = vec![vec![1i16, 2], Vec::new(), vec![-3, 4, 5]];
    let mut buf = vec![0u8; 128];
    let cap = buf.len();
    let written = {
        let mut p = MemoryPacker::new(&mut buf);
        p.write_nested_value_list(Some(&nested));
        cap - p.remaining_len()
    };

    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf[..written], &mut pool);
    let got: Vec<Vec<i16>> = u.read_nested_value_list().expect("nested values");
    assert_eq!(got, nested);
    assert_eq!(u.remaining_data(), 0);
}

/// Small object used to cover object packing helpers without generated IPC types.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct TestObject {
    /// Signed field to prove multi-byte values keep little-endian layout.
    value: i32,
    /// One-byte field that follows immediately after [`Self::value`].
    tag: u8,
}

impl MemoryPackable for TestObject {
    fn pack(&mut self, packer: &mut MemoryPacker<'_>) {
        packer.write(&self.value);
        packer.write(&self.tag);
    }

    fn unpack<P: renderide_shared::memory_packer_entity_pool::MemoryPackerEntityPool>(
        &mut self,
        unpacker: &mut MemoryUnpacker<'_, '_, P>,
    ) -> Result<(), WireDecodeError> {
        self.value = unpacker.read::<i32>()?;
        self.tag = unpacker.read::<u8>()?;
        Ok(())
    }
}

#[test]
fn pack_unpack_required_optional_and_object_lists() {
    let mut required = TestObject { value: -42, tag: 7 };
    let mut optional = TestObject { value: 99, tag: 3 };
    let mut list = [
        TestObject { value: 1, tag: 10 },
        TestObject { value: 2, tag: 20 },
    ];
    let mut buf = vec![0u8; 128];
    let cap = buf.len();
    let written = {
        let mut p = MemoryPacker::new(&mut buf);
        p.write_object_required(&mut required);
        p.write_object::<TestObject>(None);
        p.write_object(Some(&mut optional));
        p.write_object_list(Some(&mut list));
        cap - p.remaining_len()
    };

    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf[..written], &mut pool);
    let mut decoded_required = TestObject::default();
    u.read_object_required(&mut decoded_required)
        .expect("required object");
    let decoded_none: Option<TestObject> = u.read_object().expect("none");
    let decoded_optional: Option<TestObject> = u.read_object().expect("optional");
    let decoded_list: Vec<TestObject> = u.read_object_list().expect("object list");

    assert_eq!(decoded_required, required);
    assert_eq!(decoded_none, None);
    assert_eq!(decoded_optional, Some(optional));
    assert_eq!(decoded_list, list);
    assert_eq!(u.remaining_data(), 0);
}

#[test]
fn read_value_list_negative_count_yields_empty_vec() {
    let mut buf = [0u8; 4];
    {
        let mut p = MemoryPacker::new(&mut buf);
        p.write(&-1i32);
    };
    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf, &mut pool);
    let got: Vec<u8> = u.read_value_list().expect("decoded");
    assert!(
        got.is_empty(),
        "negative outer count must defensively decode as empty"
    );
}

#[test]
fn read_string_list_giant_count_returns_underrun_without_oom() {
    let mut buf = [0u8; 4];
    {
        let mut p = MemoryPacker::new(&mut buf);
        p.write(&1_000_000i32);
    };
    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf, &mut pool);
    let err = u.read_string_list().unwrap_err();
    assert!(
        matches!(err, MemoryUnpackError::Underrun { .. }),
        "alloc_hint must cap the speculative Vec capacity so we hit Underrun before OOM (got {err:?})"
    );
}

#[test]
fn packed_bools_tuple_getters_return_leading_n_bits_for_alternating_pattern() {
    let p = PackedBools::from_byte(0xAA);
    assert_eq!(p.two(), (false, true));
    assert_eq!(p.three(), (false, true, false));
    assert_eq!(p.four(), (false, true, false, true));
    assert_eq!(p.five(), (false, true, false, true, false));
    assert_eq!(p.six(), (false, true, false, true, false, true));
    assert_eq!(p.seven(), (false, true, false, true, false, true, false));
}

#[test]
fn packed_bools_tuple_getters_match_zero_byte() {
    let p = PackedBools::from_byte(0);
    assert_eq!(p.two(), (false, false));
    assert_eq!(p.seven(), (false, false, false, false, false, false, false));
}

#[test]
fn read_str_rejects_length_above_max() {
    let mut buf = [0u8; 4];
    {
        let mut p = MemoryPacker::new(&mut buf);
        let oversized = (MAX_STRING_LEN + 1) as i32;
        p.write(&oversized);
    };
    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf, &mut pool);
    let err = u.read_str().unwrap_err();
    match err {
        MemoryUnpackError::StringTooLong { requested, max } => {
            assert_eq!(requested, MAX_STRING_LEN + 1);
            assert_eq!(max, MAX_STRING_LEN);
        }
        other => panic!("expected StringTooLong, got {other:?}"),
    }
}
