//! Byte ring view over the shared mapping after [`crate::layout::QueueHeader`].
//!
//! Logical offsets may be negative or larger than `capacity`; they are reduced with
//! Euclidean modulo before indexing. Cross-process ordering for message bodies is enforced by
//! [`crate::layout::MessageHeader::state`] and the shared [`crate::layout::QueueHeader`] atomics;
//! this type therefore exposes mutating helpers through `&self` while holding a `*mut u8` base.

use std::sync::atomic::Ordering;

use crate::layout::MessageHeader;

/// View of the ring bytes (`capacity` is the user ring length only; excludes [`crate::layout::QueueHeader`]).
#[derive(Copy, Clone)]
pub struct RingView {
    /// Base pointer to the first byte of the ring (immediately after the queue header in the mapping).
    ptr: *mut u8,
    /// Ring length in bytes (matches [`crate::QueueOptions::capacity`]).
    capacity: i64,
}

/// # Safety
///
/// `ptr` must be valid for reads and writes for `capacity` bytes for the lifetime of queue usage,
/// and `capacity` must be positive. The pointer must refer to the ring region inside the mapping
/// opened by [`crate::memory::SharedMapping::open_queue`].
// SAFETY: see the doc comment above -- the pointer is valid for the lifetime of queue usage.
unsafe impl Send for RingView {}

/// # Safety
///
/// All synchronisation for queue data races is provided by atomics in the wire format and by
/// single-writer / single-reader protocol on message bodies; concurrent raw access is allowed
/// only through those contracts.
// SAFETY: see the doc comment above -- all data-race synchronization is handled by the wire protocol.
unsafe impl Sync for RingView {}

impl RingView {
    /// Wraps a raw ring base pointer and capacity.
    ///
    /// # Safety
    ///
    /// See [`RingView`] type-level safety requirements. `capacity` must match the options used to
    /// open the mapping.
    pub(crate) unsafe fn from_raw(ptr: *mut u8, capacity: i64) -> Self {
        debug_assert!(capacity > 0, "ring capacity must be positive");
        Self { ptr, capacity }
    }

    /// Message header at the logical start of the current slot, or [`None`] if the resulting
    /// pointer does not satisfy [`MessageHeader`]'s alignment.
    ///
    /// The wire protocol requires the eight-byte [`MessageHeader`] to lie in contiguous physical
    /// bytes at `(logical_offset % capacity)` and slots are aligned to eight bytes. A buggy or
    /// hostile peer that writes a slot at a non-aligned offset would otherwise drive an
    /// undefined-behavior read here; we surface [`None`] so the subscriber can drain the queue
    /// instead of dereferencing a misaligned reference.
    ///
    /// # Safety
    ///
    /// Callers must only use offsets produced by the publisher after a successful space check.
    /// The base pointer must remain valid for the lifetime of the queue (see the type-level
    /// `# Safety` on [`RingView`]).
    pub(crate) unsafe fn message_header_at(&self, logical_offset: i64) -> Option<&MessageHeader> {
        // SAFETY: caller upholds the same offset and mapping contract as `message_header_ptr`.
        let header_ptr = unsafe { self.message_header_ptr(logical_offset) }?;
        // SAFETY: alignment was verified; the eight-byte header lies contiguously in the live
        // ring at this physical offset.
        Some(unsafe { &*header_ptr })
    }

    /// Aligned pointer to the message header at `logical_offset`, or [`None`] when misaligned.
    ///
    /// Use this when a field other than [`MessageHeader::state`] must be written. Forming
    /// [`&MessageHeader`] and assigning `body_length` is rejected because that field is not
    /// atomic, and holding the shared reference across the write would alias the slot.
    ///
    /// # Safety
    ///
    /// Same contract as [`Self::message_header_at`].
    pub(crate) unsafe fn message_header_ptr(
        &self,
        logical_offset: i64,
    ) -> Option<*mut MessageHeader> {
        let phys = (logical_offset.rem_euclid(self.capacity)) as usize;
        // Compute the candidate header pointer without dereferencing it so the alignment check
        // can run before any potentially-misaligned read.
        #[expect(
            clippy::cast_ptr_alignment,
            reason = "alignment is checked with is_aligned before the MessageHeader pointer is dereferenced"
        )]
        // SAFETY: `phys < capacity` by Euclidean modulo on a positive `capacity`; the ring base
        // is valid for `capacity` bytes per the `RingView` type-level invariant.
        let header_ptr = unsafe { self.ptr.add(phys) }.cast::<MessageHeader>();
        if !header_ptr.is_aligned() {
            return None;
        }
        Some(header_ptr)
    }

    /// Copies `len` bytes starting at logical `offset` into a new vector.
    #[cfg(test)]
    pub(crate) fn read(self, offset: i64, len: usize) -> Vec<u8> {
        let mut result = Vec::new();
        self.read_into(offset, len, &mut result);
        result
    }

    /// Copies `len` bytes starting at logical `offset` into `out`, reusing its allocation.
    ///
    /// `out` is cleared first. When `len > 0` the bytes are written directly into the spare
    /// capacity so the buffer is not zero-filled before the copy.
    pub(crate) fn read_into(self, offset: i64, len: usize, out: &mut Vec<u8>) {
        out.clear();
        if len == 0 {
            return;
        }
        out.reserve(len);
        let segments = WrappedSegments::new(offset, self.capacity, len);
        // SAFETY: `reserve` made `len` bytes of spare capacity available at `out.as_mut_ptr()`.
        // `copy_from_ring_raw` initializes every one of those bytes before `set_len`.
        unsafe {
            segments.copy_from_ring_raw(self.ptr, out.as_mut_ptr());
        }
        // SAFETY: `copy_from_ring_raw` initialized every one of those bytes before `set_len`.
        unsafe {
            out.set_len(len);
        }
    }

    /// Writes `data` at logical `offset`, wrapping at `capacity`.
    pub(crate) fn write(self, offset: i64, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let segments = WrappedSegments::new(offset, self.capacity, data.len());
        segments.copy_to_ring(self.ptr, data);
    }

    /// Zero-fills `len` bytes at logical `offset`, wrapping at `capacity`.
    pub(crate) fn clear(self, offset: i64, len: usize) {
        if len == 0 {
            return;
        }
        let segments = WrappedSegments::new(offset, self.capacity, len);
        segments.clear_ring(self.ptr);
    }
}

/// One or two physical byte spans for a logical ring range.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct WrappedSegments {
    /// Physical offset of the first span.
    phys: usize,
    /// Length of the span beginning at [`Self::phys`].
    first: usize,
    /// Length of the wrapped span beginning at the ring base.
    second: usize,
}

impl WrappedSegments {
    /// Builds wrapped physical spans for `len` bytes at logical `offset`.
    fn new(offset: i64, capacity: i64, len: usize) -> Self {
        debug_assert!(capacity > 0, "capacity must be positive");
        let cap = capacity as usize;
        let phys = (offset.rem_euclid(capacity)) as usize;
        let first = (cap - phys).min(len);
        let second = len - first;
        Self {
            phys,
            first,
            second,
        }
    }

    /// Copies the described bytes from `ring_ptr` into `dst`.
    ///
    /// # Safety
    ///
    /// `dst` must be valid for writes of `first + second` bytes. Those bytes are fully
    /// initialized by this call and may start uninitialized.
    unsafe fn copy_from_ring_raw(self, ring_ptr: *mut u8, dst: *mut u8) {
        if self.first > 0 {
            // SAFETY: `new` guarantees `phys + first <= capacity`; the ring region is live and
            // readable for `capacity` bytes per the `RingView` type invariant.
            let src = unsafe { ring_ptr.add(self.phys) };
            // SAFETY: `dst` is valid for `first` bytes by the caller contract.
            unsafe {
                std::ptr::copy_nonoverlapping(src, dst, self.first);
            }
        }
        if self.second > 0 {
            // SAFETY: `new` guarantees the wrapped segment begins at the ring base and fits the
            // caller-constrained logical range. `dst.add(first)` is in bounds by the caller
            // contract covering `first + second` bytes.
            let dst_tail = unsafe { dst.add(self.first) };
            // SAFETY: `dst_tail` is in bounds by the caller contract covering `first + second` bytes.
            unsafe {
                std::ptr::copy_nonoverlapping(ring_ptr, dst_tail, self.second);
            }
        }
    }

    /// Copies `data` into the described bytes at `ring_ptr`.
    fn copy_to_ring(self, ring_ptr: *mut u8, data: &[u8]) {
        if self.first > 0 {
            // SAFETY: `new` guarantees `phys + first <= capacity`; `data[..first]` and the ring
            // region do not alias. The single-writer wire protocol guards writes to this slot.
            let dst = unsafe { ring_ptr.add(self.phys) };
            // SAFETY: `data[..first]` and the ring region do not alias. The single-writer wire
            // protocol guards writes to this slot.
            unsafe {
                std::ptr::copy_nonoverlapping(data.as_ptr(), dst, self.first);
            }
        }
        if self.second > 0 {
            // SAFETY: `new` guarantees the wrapped segment starts at the ring base; source and
            // destination are distinct allocations under the queue wire protocol.
            let src = unsafe { data.as_ptr().add(self.first) };
            // SAFETY: source and destination are distinct allocations under the queue wire protocol.
            unsafe {
                std::ptr::copy_nonoverlapping(src, ring_ptr, self.second);
            }
        }
    }

    /// Zero-fills the described bytes at `ring_ptr`.
    fn clear_ring(self, ring_ptr: *mut u8) {
        if self.first > 0 {
            // SAFETY: `new` guarantees `phys + first <= capacity`; the slot is guarded by the
            // single-reader consumption protocol.
            let dst = unsafe { ring_ptr.add(self.phys) };
            // SAFETY: the slot is guarded by the single-reader consumption protocol.
            unsafe {
                std::ptr::write_bytes(dst, 0, self.first);
            }
        }
        if self.second > 0 {
            // SAFETY: `new` guarantees the wrapped segment starts at the ring base and fits the
            // caller-constrained logical range.
            unsafe {
                std::ptr::write_bytes(ring_ptr, 0, self.second);
            }
        }
    }
}

/// Returns free bytes in the ring for a new message, or `0` when full.
///
/// When [`crate::layout::QueueHeader::read_offset`] equals [`crate::layout::QueueHeader::write_offset`],
/// the queue is empty and the full `capacity` is available.
pub fn available_space(header: &crate::layout::QueueHeader, capacity: i64) -> i64 {
    if capacity <= 0 {
        return 0;
    }
    // Acquire pairs with the subscriber's Release store of `read_offset` and the publisher's
    // SeqCst `write_offset` CAS, which is also a release store. Sequentially-consistent ordering
    // is not required to publish freed space or a newly claimed slot.
    let read_off = header.read_offset.load(Ordering::Acquire);
    let write_off = header.write_offset.load(Ordering::Acquire);
    if read_off == write_off {
        return capacity;
    }
    let read_phys = read_off.rem_euclid(capacity);
    let write_phys = write_off.rem_euclid(capacity);
    if read_phys == write_phys {
        return 0;
    }
    let free = if read_phys < write_phys {
        capacity - write_phys + read_phys
    } else {
        read_phys - write_phys
    };
    free.clamp(0, capacity)
}

#[cfg(test)]
mod tests {
    //! # Safety (tests)
    //!
    //! All `unsafe` calls below operate on caller-owned local stack buffers (`buf`) whose lifetime
    //! exceeds the `RingView` and which are not aliased by any other thread for the duration of
    //! the test. `capacity` matches the buffer length. `message_header_at(0)` reads/writes the
    //! first eight bytes of `buf`, which fit entirely inside the allocation.
    use std::mem::size_of;
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::layout::QueueHeader;

    #[test]
    fn split_no_wrap() {
        let segments = WrappedSegments::new(2, 10, 3);
        assert_eq!(
            segments,
            WrappedSegments {
                phys: 2,
                first: 3,
                second: 0,
            }
        );
    }

    #[test]
    fn split_exact_end_then_wrap() {
        let segments = WrappedSegments::new(8, 10, 4);
        assert_eq!(
            segments,
            WrappedSegments {
                phys: 8,
                first: 2,
                second: 2,
            }
        );
    }

    #[test]
    fn split_full_second_segment() {
        let segments = WrappedSegments::new(0, 6, 6);
        assert_eq!(
            segments,
            WrappedSegments {
                phys: 0,
                first: 6,
                second: 0,
            }
        );
    }

    #[test]
    fn split_negative_logical_offset() {
        let segments = WrappedSegments::new(-2, 5, 4);
        assert_eq!(segments.phys, 3);
        assert_eq!(segments.first + segments.second, 4);
    }

    #[test]
    fn write_read_roundtrip_wrap() {
        let mut buf = [0u8; 6];
        let cap = 6i64;
        // SAFETY: see module `# Safety (tests)` -- `buf` outlives `ring`, `cap` matches length.
        let ring = unsafe { RingView::from_raw(buf.as_mut_ptr(), cap) };
        ring.write(4, &[1, 2, 3]);
        let got = ring.read(4, 3);
        assert_eq!(got, vec![1, 2, 3]);
        assert_eq!(buf[4], 1);
        assert_eq!(buf[5], 2);
        assert_eq!(buf[0], 3);
    }

    #[test]
    fn read_into_reuses_capacity_without_keeping_stale_bytes() {
        let buf = [1u8, 2, 3, 4];
        // SAFETY: read-only test; `buf` outlives `ring`, capacity matches length.
        let ring = unsafe { RingView::from_raw(buf.as_ptr().cast_mut(), 4) };
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(b"stale!!");
        let capacity = out.capacity();
        ring.read_into(1, 3, &mut out);
        assert_eq!(out, vec![2, 3, 4]);
        assert!(out.capacity() >= capacity);
        ring.read_into(0, 0, &mut out);
        assert!(out.is_empty());
        assert!(out.capacity() >= capacity);
    }

    #[test]
    fn read_zero_len_returns_empty() {
        let buf = [9u8; 4];
        // SAFETY: read-only test; `buf` outlives `ring`, capacity matches length.
        let ring = unsafe { RingView::from_raw(buf.as_ptr().cast_mut(), 4) };
        let got = ring.read(0, 0);
        assert!(got.is_empty());
    }

    #[test]
    fn write_empty_is_noop() {
        let mut buf = [7u8; 4];
        // SAFETY: see module `# Safety (tests)` -- `buf` outlives `ring`, capacity matches length.
        let ring = unsafe { RingView::from_raw(buf.as_mut_ptr(), 4) };
        ring.write(2, &[]);
        assert_eq!(buf, [7u8; 4]);
    }

    #[test]
    fn clear_zero_len_is_noop() {
        let mut buf = [5u8; 4];
        // SAFETY: see module `# Safety (tests)` -- `buf` outlives `ring`, capacity matches length.
        let ring = unsafe { RingView::from_raw(buf.as_mut_ptr(), 4) };
        ring.clear(0, 0);
        assert_eq!(buf, [5u8; 4]);
    }

    #[test]
    fn read_spans_wrap_when_offset_near_capacity_end() {
        let buf = [10u8, 20u8, 30u8, 40u8, 50u8];
        // SAFETY: read-only test; `buf` outlives `ring`, capacity matches length.
        let ring = unsafe { RingView::from_raw(buf.as_ptr().cast_mut(), 5) };
        let got = ring.read(3, 4);
        assert_eq!(got, vec![40u8, 50u8, 10u8, 20u8]);
    }

    #[test]
    fn write_spans_wrap_from_negative_logical_offset() {
        let mut buf = [0u8; 5];
        // SAFETY: see module `# Safety (tests)` -- `buf` outlives `ring`, capacity matches length.
        let ring = unsafe { RingView::from_raw(buf.as_mut_ptr(), 5) };
        ring.write(-2, &[1, 2, 3, 4]);
        assert_eq!(buf, [3, 4, 0, 1, 2]);
    }

    #[test]
    fn clear_spans_wrap() {
        let mut buf = [9u8; 6];
        // SAFETY: see module `# Safety (tests)` -- `buf` outlives `ring`, capacity matches length.
        let ring = unsafe { RingView::from_raw(buf.as_mut_ptr(), 6) };
        ring.clear(4, 4);
        assert_eq!(buf, [0u8, 0u8, 9u8, 9u8, 0u8, 0u8]);
    }

    #[test]
    fn read_full_ring_length() {
        let buf = [1u8, 2, 3, 4, 5, 6];
        // SAFETY: read-only test; `buf` outlives `ring`, capacity matches length.
        let ring = unsafe { RingView::from_raw(buf.as_ptr().cast_mut(), 6) };
        let got = ring.read(0, 6);
        assert_eq!(got, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn available_space_empty_queue() {
        let h = QueueHeader::default();
        assert_eq!(available_space(&h, 64), 64);
    }

    #[test]
    fn available_space_when_physically_full() {
        let h = QueueHeader::default();
        h.read_offset.store(0, Ordering::SeqCst);
        h.write_offset.store(8, Ordering::SeqCst);
        assert_eq!(available_space(&h, 8), 0);
    }

    #[test]
    fn available_space_in_use() {
        let h = QueueHeader::default();
        h.read_offset.store(0, Ordering::SeqCst);
        h.write_offset.store(8, Ordering::SeqCst);
        assert_eq!(available_space(&h, 24), 24 - 8);
    }

    #[test]
    fn message_header_at_reads_state() {
        use crate::layout::{MessageHeader, STATE_WRITING};

        // 64-byte buffer aligned to MessageHeader so `as_mut_ptr()` lands on a 4-byte boundary
        // and `message_header_at(0)` returns `Some` rather than rejecting on alignment.
        #[repr(align(8))]
        struct AlignedBuf([u8; 64]);
        let mut storage = AlignedBuf([0u8; 64]);
        let buf = &mut storage.0;
        // SAFETY: see module `# Safety (tests)` -- `buf` outlives `ring`, capacity matches length.
        let ring = unsafe { RingView::from_raw(buf.as_mut_ptr(), 64) };
        // SAFETY: offset 0 is a valid 8-byte header slot inside the 64-byte buffer.
        let mh = unsafe { ring.message_header_at(0) }.expect("aligned");
        mh.state.store(STATE_WRITING, Ordering::SeqCst);
        // SAFETY: same slot, unchanged layout.
        let mh2 = unsafe { ring.message_header_at(0) }.expect("aligned");
        assert_eq!(mh2.state.load(Ordering::SeqCst), STATE_WRITING);
        assert_eq!(size_of::<MessageHeader>(), 8);
    }

    #[test]
    fn message_header_at_returns_none_when_misaligned() {
        // Build a buffer whose start is aligned but offset 1 is not.
        #[repr(align(8))]
        struct AlignedBuf([u8; 64]);
        let mut storage = AlignedBuf([0u8; 64]);
        let buf = &mut storage.0;
        // SAFETY: see module `# Safety (tests)` -- `buf` outlives `ring`, capacity matches length.
        let ring = unsafe { RingView::from_raw(buf.as_mut_ptr(), 64) };
        // Logical offset 1 produces phys=1, which is not 4-byte aligned.
        // SAFETY: read-only path, no dereference because alignment guard returns None first.
        assert!(unsafe { ring.message_header_at(1) }.is_none());
        // 4-byte aligned offsets pass the guard.
        // SAFETY: same buffer, alignment satisfied.
        assert!(unsafe { ring.message_header_at(4) }.is_some());
    }
}
