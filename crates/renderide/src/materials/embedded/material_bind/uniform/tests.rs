use std::num::{NonZeroU64, NonZeroUsize};

use hashbrown::HashSet;

use super::super::key::MaterialUniformCacheKey;
use super::MaterialUniformArenaAllocator;

fn key(material_asset_id: i32) -> MaterialUniformCacheKey {
    MaterialUniformCacheKey {
        stem_hash: 7,
        material_asset_id,
        property_block_slot0: None,
        renderer_property_block_id: None,
        texture_2d_asset_id: -1,
        shader_variant_bits: None,
        shader_source_generation: 0,
        texture_residency_generation: 0,
    }
}

fn key_with_block(material_asset_id: i32, property_block_id: i32) -> MaterialUniformCacheKey {
    MaterialUniformCacheKey {
        stem_hash: 7,
        material_asset_id,
        property_block_slot0: Some(property_block_id),
        renderer_property_block_id: None,
        texture_2d_asset_id: -1,
        shader_variant_bits: None,
        shader_source_generation: 0,
        texture_residency_generation: 0,
    }
}

#[test]
fn arena_allocator_aligns_offsets() {
    let mut allocator = MaterialUniformArenaAllocator::new(1024, 4096, 256);
    let size = NonZeroU64::new(80).unwrap();

    let a = allocator.resolve_slot(key(1), size, 0, 0).unwrap();
    let b = allocator.resolve_slot(key(2), size, 0, 0).unwrap();

    assert_eq!(a.offset, 0);
    assert_eq!(b.offset, 256);
}

#[test]
fn arena_allocator_property_change_rewrites_slot_without_new_buffer() {
    let mut allocator = MaterialUniformArenaAllocator::new(1024, 4096, 256);
    let size = NonZeroU64::new(64).unwrap();
    let first = allocator.resolve_slot(key(1), size, 1, 0).unwrap();
    allocator.mark_written(&key(1), first.buffer_generation, 1, 0);

    let updated = allocator.resolve_slot(key(1), size, 2, 0).unwrap();

    assert!(updated.needs_write);
    assert_eq!(updated.offset, first.offset);
    assert_eq!(updated.buffer_generation, first.buffer_generation);
    assert_eq!(allocator.generation(), first.buffer_generation);
}

#[test]
fn arena_allocator_reuses_stable_slot() {
    let mut allocator = MaterialUniformArenaAllocator::new(1024, 4096, 256);
    let size = NonZeroU64::new(80).unwrap();

    let first = allocator.resolve_slot(key(1), size, 3, 5).unwrap();
    allocator.mark_written(&key(1), first.buffer_generation, 3, 5);
    let second = allocator.resolve_slot(key(1), size, 3, 5).unwrap();

    assert_eq!(first.offset, second.offset);
    assert!(!second.needs_write);
}

#[test]
fn arena_allocator_growth_invalidates_existing_slots_for_new_generation() {
    let mut allocator = MaterialUniformArenaAllocator::new(256, 2048, 256);
    let size = NonZeroU64::new(128).unwrap();

    let first = allocator.resolve_slot(key(1), size, 1, 1).unwrap();
    allocator.mark_written(&key(1), first.buffer_generation, 1, 1);
    assert_eq!(allocator.generation(), 0);

    let _second = allocator.resolve_slot(key(2), size, 1, 1).unwrap();
    assert_eq!(allocator.generation(), 1);
    let first_after_growth = allocator.resolve_slot(key(1), size, 1, 1).unwrap();

    assert_eq!(first_after_growth.offset, first.offset);
    assert_eq!(first_after_growth.buffer_generation, 1);
    assert!(first_after_growth.needs_write);
}

#[test]
fn arena_allocator_purges_material_and_property_block_slots() {
    let mut allocator = MaterialUniformArenaAllocator::new(1024, 4096, 256);
    let size = NonZeroU64::new(80).unwrap();
    allocator.resolve_slot(key(1), size, 0, 0).unwrap();
    allocator
        .resolve_slot(key_with_block(2, 20), size, 0, 0)
        .unwrap();
    allocator.resolve_slot(key(3), size, 0, 0).unwrap();

    let mut materials = HashSet::new();
    materials.insert(1);
    let mut blocks = HashSet::new();
    blocks.insert(20);
    assert_eq!(allocator.purge_material_assets(&materials, &blocks), 2);

    assert!(!allocator.slots.contains(&key(1)));
    assert!(!allocator.slots.contains(&key_with_block(2, 20)));
    assert!(allocator.slots.contains(&key(3)));
}

#[test]
fn uniform_key_tracks_shader_generation_and_texture_residency() {
    let base = key(1);
    let mut generation = base;
    generation.shader_source_generation = 4;
    let mut residency = base;
    residency.texture_residency_generation = 9;

    assert_ne!(base, generation);
    assert_ne!(base, residency);
}

#[test]
fn reinstall_drops_old_generation_uniform_slots() {
    let mut allocator =
        MaterialUniformArenaAllocator::with_slot_capacity(1024, 4096, 256, nonzero_slots(4));
    let size = NonZeroU64::new(80).unwrap();
    let mut old = key(1);
    old.shader_source_generation = 1;
    let mut current = key(1);
    current.shader_source_generation = 2;
    let other_stem = MaterialUniformCacheKey {
        stem_hash: 8,
        shader_source_generation: 1,
        ..key(1)
    };

    allocator.resolve_slot(old, size, 0, 0).unwrap();
    allocator.resolve_slot(current, size, 0, 0).unwrap();
    allocator.resolve_slot(other_stem, size, 0, 0).unwrap();

    assert_eq!(allocator.evict_stem_generation(old.stem_hash, 1), 1);
    assert!(!allocator.slots.contains(&old));
    assert!(allocator.slots.contains(&current));
    assert!(allocator.slots.contains(&other_stem));
    assert_eq!(allocator.eviction_count(), 1);
}

#[test]
fn uniform_slot_lru_cap_holds() {
    let mut allocator =
        MaterialUniformArenaAllocator::with_slot_capacity(4096, 8192, 64, nonzero_slots(2));
    let size = NonZeroU64::new(64).unwrap();

    allocator.resolve_slot(key(1), size, 0, 0).unwrap();
    allocator.resolve_slot(key(2), size, 0, 0).unwrap();
    allocator.resolve_slot(key(3), size, 0, 0).unwrap();

    assert_eq!(allocator.slot_count(), 2);
    assert!(!allocator.slots.contains(&key(1)));
    assert!(allocator.slots.contains(&key(2)));
    assert!(allocator.slots.contains(&key(3)));
    assert_eq!(allocator.eviction_count(), 1);
}

#[test]
fn mutation_rewrites_slot_without_new_offset_or_generation() {
    let mut allocator = MaterialUniformArenaAllocator::new(1024, 4096, 256);
    let size = NonZeroU64::new(64).unwrap();
    let first = allocator.resolve_slot(key(1), size, 1, 0).unwrap();
    allocator.mark_written(&key(1), first.buffer_generation, 1, 0);

    let updated = allocator.resolve_slot(key(1), size, 2, 0).unwrap();

    assert!(updated.needs_write);
    assert_eq!(updated.offset, first.offset);
    assert_eq!(updated.buffer_generation, first.buffer_generation);
    assert_eq!(allocator.generation(), 0);
    assert_eq!(allocator.capacity(), 1024);
}

#[test]
fn evicted_offset_not_reused_until_frame_retires() {
    let mut allocator =
        MaterialUniformArenaAllocator::with_slot_capacity(1024, 8192, 256, nonzero_slots(4));
    let size = NonZeroU64::new(80).unwrap();
    allocator.begin_frame();
    let first = allocator.resolve_slot(key(1), size, 0, 0).unwrap();
    assert_eq!(first.offset, 0);
    for id in 2..=4 {
        allocator.resolve_slot(key(id), size, 0, 0).unwrap();
    }
    assert_eq!(allocator.capacity(), 1024);
    assert_eq!(allocator.generation(), 0);

    // The table is full and the bump cursor is at capacity, so the insert reclaims the LRU
    // offset. That offset stays pending until this frame's submit completes.
    let same_frame = allocator.resolve_slot(key(5), size, 0, 0).unwrap();
    assert_ne!(same_frame.offset, first.offset);
    assert!(!allocator.slots.contains(&key(1)));
    assert!(allocator.capacity() > 1024);

    let capacity = allocator.capacity();
    let generation = allocator.generation();
    allocator.retire_submitted_frame();
    allocator.begin_frame();
    let reused = allocator.resolve_slot(key(6), size, 0, 0).unwrap();
    assert_eq!(reused.offset, first.offset);
    assert_eq!(allocator.capacity(), capacity);
    assert_eq!(allocator.generation(), generation);
}

#[test]
fn churn_keeps_arena_capacity_bounded() {
    let mut allocator =
        MaterialUniformArenaAllocator::with_slot_capacity(1024, 65_536, 256, nonzero_slots(4));
    let size = NonZeroU64::new(64).unwrap();
    let mut settled_capacity = 0;
    let mut settled_generation = 0;

    for frame in 0..32u32 {
        if frame > 0 {
            allocator.retire_submitted_frame();
        }
        allocator.begin_frame();
        for slot in 0..4u32 {
            let id = i32::try_from(frame * 4 + slot).unwrap_or(i32::MAX);
            allocator.resolve_slot(key(id), size, 0, 0).unwrap();
        }
        if frame == 1 {
            settled_capacity = allocator.capacity();
            settled_generation = allocator.generation();
        }
    }

    assert_eq!(allocator.slot_count(), 4);
    assert!(settled_capacity <= 2048);
    assert_eq!(allocator.capacity(), settled_capacity);
    assert_eq!(allocator.generation(), settled_generation);
}

#[test]
fn reallocation_bumps_generation_when_free_span_cannot_fit() {
    let mut allocator =
        MaterialUniformArenaAllocator::with_slot_capacity(512, 8192, 256, nonzero_slots(8));
    let small = NonZeroU64::new(64).unwrap();
    allocator.resolve_slot(key(1), small, 0, 0).unwrap();
    allocator.resolve_slot(key(2), small, 0, 0).unwrap();
    let mut drop_first = HashSet::new();
    drop_first.insert(1);
    assert_eq!(
        allocator.purge_material_assets(&drop_first, &HashSet::new()),
        1
    );
    assert_eq!(allocator.generation(), 0);
    let before_capacity = allocator.capacity();

    let allocated = allocator
        .resolve_slot(key(3), NonZeroU64::new(400).unwrap(), 0, 0)
        .unwrap();

    assert!(allocator.generation() > 0);
    assert!(allocator.capacity() > before_capacity);
    assert_eq!(allocated.buffer_generation, allocator.generation());
    assert!(allocated.needs_write);
}

#[test]
fn compaction_bumps_generation_without_growing() {
    let mut allocator =
        MaterialUniformArenaAllocator::with_slot_capacity(1024, 1024, 256, nonzero_slots(8));
    let small = NonZeroU64::new(80).unwrap();
    let first = allocator.resolve_slot(key(1), small, 1, 1).unwrap();
    allocator.mark_written(&key(1), first.buffer_generation, 1, 1);
    allocator.resolve_slot(key(2), small, 1, 1).unwrap();
    let third = allocator.resolve_slot(key(3), small, 1, 1).unwrap();
    allocator.mark_written(&key(3), third.buffer_generation, 1, 1);
    let mut drop_middle = HashSet::new();
    drop_middle.insert(2);
    assert_eq!(
        allocator.purge_material_assets(&drop_middle, &HashSet::new()),
        1
    );
    assert_eq!(allocator.generation(), 0);

    let allocated = allocator
        .resolve_slot(key(4), NonZeroU64::new(300).unwrap(), 0, 0)
        .unwrap();

    assert_eq!(allocator.capacity(), 1024);
    assert_eq!(allocator.generation(), 1);
    assert_eq!(allocated.buffer_generation, 1);
    let repacked = allocator.resolve_slot(key(3), small, 1, 1).unwrap();
    assert!(repacked.needs_write);
    assert_eq!(repacked.buffer_generation, 1);
    assert!(repacked.offset < third.offset);
}

#[test]
fn uniform_key_covers_pr24_material_inputs() {
    let base = MaterialUniformCacheKey {
        property_block_slot0: Some(2),
        renderer_property_block_id: Some(3),
        texture_2d_asset_id: 4,
        shader_variant_bits: Some(1),
        shader_source_generation: 5,
        texture_residency_generation: 6,
        ..key(1)
    };

    assert_ne!(
        base,
        MaterialUniformCacheKey {
            material_asset_id: 2,
            ..base
        }
    );
    assert_ne!(
        base,
        MaterialUniformCacheKey {
            property_block_slot0: Some(9),
            ..base
        }
    );
    assert_ne!(
        base,
        MaterialUniformCacheKey {
            renderer_property_block_id: Some(8),
            ..base
        }
    );
    assert_ne!(
        base,
        MaterialUniformCacheKey {
            texture_2d_asset_id: 5,
            ..base
        }
    );
    assert_ne!(
        base,
        MaterialUniformCacheKey {
            shader_variant_bits: Some(2),
            ..base
        }
    );
    assert_ne!(
        base,
        MaterialUniformCacheKey {
            shader_source_generation: 6,
            ..base
        }
    );
    assert_ne!(
        base,
        MaterialUniformCacheKey {
            texture_residency_generation: 7,
            ..base
        }
    );
}

fn nonzero_slots(cap: usize) -> NonZeroUsize {
    NonZeroUsize::new(cap).unwrap_or(NonZeroUsize::MIN)
}
