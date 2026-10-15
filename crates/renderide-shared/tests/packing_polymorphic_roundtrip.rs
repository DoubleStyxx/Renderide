//! Integration: roundtrip [`MemoryPacker::write_polymorphic_list`] through
//! [`MemoryUnpacker::read_polymorphic_list`] using a small in-test sum type.

use renderide_shared::WireDecodeError;
use renderide_shared::default_entity_pool::DefaultEntityPool;
use renderide_shared::memory_packer::MemoryPacker;
use renderide_shared::memory_unpacker::MemoryUnpacker;
use renderide_shared::polymorphic_memory_packable_entity::PolymorphicEncode;

/// Two-variant sum type encoding `Foo(i32)` as discriminant `0` and `Bar(i32)` as discriminant `1`.
///
/// Both variants use the same payload type so the on-the-wire payload size matches the test's
/// expectations and Rust's `variant_size_differences` lint stays clean for this fixture.
#[derive(Debug, PartialEq, Eq)]
enum TestVariant {
    /// Carries one signed 32-bit value (discriminant 0).
    Foo(i32),
    /// Carries one signed 32-bit value (discriminant 1).
    Bar(i32),
}

impl PolymorphicEncode for TestVariant {
    fn encode(&mut self, packer: &mut MemoryPacker<'_>) {
        match self {
            TestVariant::Foo(v) => {
                packer.write(&0i32);
                packer.write(v);
            }
            TestVariant::Bar(v) => {
                packer.write(&1i32);
                packer.write(v);
            }
        }
    }
}

#[test]
fn polymorphic_list_roundtrips_count_and_payload_for_mixed_variants() {
    let mut entries = vec![
        TestVariant::Foo(7),
        TestVariant::Bar(0xab),
        TestVariant::Foo(-1),
        TestVariant::Bar(i32::MIN),
    ];
    let mut buf = vec![0u8; 64];
    let cap = buf.len();
    let written = {
        let mut p = MemoryPacker::new(&mut buf);
        p.write_polymorphic_list(Some(entries.as_mut_slice()));
        cap - p.remaining_len()
    };

    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf[..written], &mut pool);
    let decoded: Vec<TestVariant> = u
        .read_polymorphic_list(|reader| {
            let tag: i32 = reader.read().map_err(WireDecodeError::from)?;
            match tag {
                0 => Ok(TestVariant::Foo(
                    reader.read().map_err(WireDecodeError::from)?,
                )),
                1 => Ok(TestVariant::Bar(
                    reader.read().map_err(WireDecodeError::from)?,
                )),
                other => panic!("unknown discriminant in test fixture: {other}"),
            }
        })
        .expect("decoded");

    assert_eq!(
        decoded,
        vec![
            TestVariant::Foo(7),
            TestVariant::Bar(0xab),
            TestVariant::Foo(-1),
            TestVariant::Bar(i32::MIN),
        ]
    );
}

#[test]
fn polymorphic_list_with_empty_input_writes_zero_count_and_decodes_to_empty() {
    let mut buf = [0u8; 4];
    let cap = buf.len();
    let written = {
        let mut p = MemoryPacker::new(&mut buf);
        p.write_polymorphic_list::<TestVariant>(None);
        cap - p.remaining_len()
    };

    let mut pool = DefaultEntityPool;
    let mut u = MemoryUnpacker::new(&buf[..written], &mut pool);
    let decoded: Vec<TestVariant> = u
        .read_polymorphic_list(|_| panic!("decode closure must not run for an empty list"))
        .expect("ok");
    assert!(decoded.is_empty());
}
