//! Consumer side of the shared-memory queue.

#![expect(
    unsafe_code,
    reason = "reads message headers through the shared ring pointer"
)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::layout::{
    MESSAGE_BODY_OFFSET, MessageHeader, QueueHeader, STATE_LOCKED, STATE_READY,
    TICKS_FOR_TEN_SECONDS, padded_message_length,
};
use crate::options::QueueOptions;
use crate::queue_resources::QueueResources;
use crate::ring::RingView;

/// `DateTime.UtcNow.Ticks` value at the Unix epoch (100 ns ticks since 0001-01-01 UTC).
const DOTNET_TICKS_AT_UNIX_EPOCH: i64 = 621_355_968_000_000_000;

/// Starting value for the contention counter in blocking [`Subscriber::dequeue`] (managed client parity).
const DEQUEUE_BACKOFF_COUNTER_INITIAL: i32 = -5;

/// After this many backoff steps, use a fixed long semaphore wait instead of ramping wait milliseconds from the counter.
const DEQUEUE_BACKOFF_HEAVY_PHASE_AFTER: i32 = 10;

/// Milliseconds for the steady semaphore wait once past [`DEQUEUE_BACKOFF_HEAVY_PHASE_AFTER`].
const DEQUEUE_BACKOFF_HEAVY_WAIT_MS: u64 = 10;

/// Pure-spin iterations on a `STATE_WRITING` slot before [`Subscriber::try_extract_message`]
/// graduates to [`std::thread::yield_now`].
const EXTRACT_SPIN_ITERATIONS: u32 = 1024;

/// Yield iterations after [`EXTRACT_SPIN_ITERATIONS`] before sleeping briefly between checks.
const EXTRACT_YIELD_ITERATIONS: u32 = 64;

/// Sleep duration between checks once both spin and yield phases are exhausted.
const EXTRACT_PARK_INTERVAL: Duration = Duration::from_micros(200);

/// Current instant in the same 100 ns tick domain as .NET `DateTime.UtcNow.Ticks`.
fn utc_now_ticks() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => {
            let since_unix_100ns = (d.as_nanos() / 100) as i64;
            since_unix_100ns.saturating_add(DOTNET_TICKS_AT_UNIX_EPOCH)
        }
        Err(_) => DOTNET_TICKS_AT_UNIX_EPOCH,
    }
}

/// Clears [`QueueHeader::read_lock_timestamp`] when a dequeue attempt completes.
struct ReadLockGuard<'a> {
    /// Header whose lock field must be released.
    header: &'a QueueHeader,
}

impl Drop for ReadLockGuard<'_> {
    fn drop(&mut self) {
        self.header.read_lock_timestamp.store(0, Ordering::SeqCst);
    }
}

/// Semaphore-backed backoff matching the managed dequeue loop.
struct DequeueBackoff {
    /// Counter carried across idle iterations (starts negative for yield-only phase).
    counter: i32,
}

impl DequeueBackoff {
    /// Builds a backoff state machine starting in the yield-heavy phase.
    const fn new() -> Self {
        Self {
            counter: DEQUEUE_BACKOFF_COUNTER_INITIAL,
        }
    }

    /// Performs one wait or yield step using `resources`' semaphore.
    fn step(&mut self, resources: &QueueResources) {
        if self.counter > DEQUEUE_BACKOFF_HEAVY_PHASE_AFTER {
            resources.wait_semaphore_timeout(Duration::from_millis(DEQUEUE_BACKOFF_HEAVY_WAIT_MS));
            return;
        }
        let old = self.counter;
        self.counter = self.counter.saturating_add(1);
        if old > 0 {
            resources.wait_semaphore_timeout(Duration::from_millis(self.counter as u64));
        } else {
            std::thread::yield_now();
        }
    }
}

/// Receives messages from the queue using the same contention and backoff pattern as the managed client.
pub struct Subscriber {
    /// Mapping, ring capacity, paired semaphore, and optional Unix file cleanup.
    res: QueueResources,
}

impl Subscriber {
    /// Opens the backing mapping and semaphore.
    ///
    /// On attach, clears [`QueueHeader::read_lock_timestamp`] to zero. The wire protocol is
    /// single-subscriber by design (the bootstrap handshake creates exactly one subscriber per
    /// queue), so a non-zero value at attach time can only be a stale lock left by a prior
    /// crashed subscriber on the same shared mapping. Without this reset, the new subscriber
    /// would wait up to [`TICKS_FOR_TEN_SECONDS`] before reclaiming on the first dequeue.
    pub fn new(options: QueueOptions) -> Result<Self, crate::OpenError> {
        let res = QueueResources::open(options)?;
        res.header().read_lock_timestamp.store(0, Ordering::SeqCst);
        Ok(Self { res })
    }

    /// Blocks until a message arrives or `cancel` is set, using semaphore-backed backoff.
    pub fn dequeue(&mut self, cancel: &AtomicBool) -> Vec<u8> {
        let mut backoff = DequeueBackoff::new();
        loop {
            if let Some(msg) = self.try_dequeue() {
                return msg;
            }
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            backoff.step(&self.res);
        }
        vec![]
    }

    /// Waits until the queue appears non-empty or `timeout` elapses.
    ///
    /// This does not remove a message; callers should follow with [`Self::try_dequeue`] or their
    /// normal drain loop. Stale semaphore tokens are tolerated by checking the queue header after
    /// every wake and continuing until the caller's deadline.
    pub fn wait_for_message_timeout(&mut self, timeout: Duration) -> bool {
        if !self.res.header().is_empty() {
            return true;
        }
        if timeout.is_zero() {
            return false;
        }
        let start = Instant::now();
        loop {
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return !self.res.header().is_empty();
            }
            let remaining = timeout.saturating_sub(elapsed);
            let acquired = self.res.wait_semaphore_timeout(remaining);
            if !self.res.header().is_empty() {
                return true;
            }
            if !acquired {
                return false;
            }
        }
    }

    /// Returns the next message if one is ready; non-blocking aside from contender spin windows.
    pub fn try_dequeue(&mut self) -> Option<Vec<u8>> {
        let mut buf = Vec::new();
        self.try_dequeue_into(&mut buf).then_some(buf)
    }

    /// Copies the next message into `buf`, reusing its allocation.
    ///
    /// Returns `false` when no message is ready. On `false`, `buf` is left unchanged. On `true`,
    /// `buf` contains exactly the payload bytes. Wire order, slot state, and read-offset
    /// advancement match [`Self::try_dequeue`].
    pub fn try_dequeue_into(&mut self, buf: &mut Vec<u8>) -> bool {
        let Some(spin_start_ticks) = self.try_acquire_read_lock() else {
            return false;
        };
        let header = self.res.header();
        let _lock = ReadLockGuard { header };
        self.try_extract_message_into(spin_start_ticks, buf)
    }

    /// Drains up to `limit` ready messages, acquiring the subscriber read lock once.
    ///
    /// `recycled` supplies payload buffers. Each buffer is cleared and filled; unused buffers
    /// stay in `recycled`. Filled buffers are appended to `out` in queue order. Passing `limit`
    /// of zero, or calling this when the queue is empty, does not take the read lock and does
    /// not modify `out`.
    ///
    /// A burst holds the crash-recovery timestamp for the whole drain instead of dropping and
    /// re-acquiring it between messages. The timestamp is refreshed if a slot is still
    /// `STATE_WRITING`, so a replacement subscriber still waits [`TICKS_FOR_TEN_SECONDS`] before
    /// reclaiming a live drain. Message bytes, padding, and the final read offset match the
    /// same number of [`Self::try_dequeue`] calls.
    pub fn try_drain_reusing(
        &mut self,
        recycled: &mut Vec<Vec<u8>>,
        out: &mut Vec<Vec<u8>>,
        limit: usize,
    ) -> usize {
        if limit == 0 {
            return 0;
        }
        let Some(spin_start_ticks) = self.try_acquire_read_lock() else {
            return 0;
        };
        let header = self.res.header();
        let _lock = ReadLockGuard { header };
        let mut drained = 0usize;
        while drained < limit {
            let mut buf = recycled.pop().unwrap_or_default();
            buf.clear();
            if !self.try_extract_message_into(spin_start_ticks, &mut buf) {
                if buf.capacity() > 0 {
                    recycled.push(buf);
                }
                break;
            }
            out.push(buf);
            drained += 1;
        }
        drained
    }

    /// Attempts to claim the subscriber read lock when the queue is non-empty and the lock is stale.
    fn try_acquire_read_lock(&self) -> Option<i64> {
        let header = self.res.header();
        if header.is_empty() {
            return None;
        }
        let ticks = utc_now_ticks();
        let read_lock = header.read_lock_timestamp.load(Ordering::SeqCst);
        if ticks - read_lock < TICKS_FOR_TEN_SECONDS {
            return None;
        }
        header
            .read_lock_timestamp
            .compare_exchange(read_lock, ticks, Ordering::SeqCst, Ordering::SeqCst)
            .ok()?;
        Some(ticks)
    }

    /// Consumes one ready message after the read lock is held.
    ///
    /// `buf` is updated only when a payload is returned. The caller supplies the tick value used
    /// for the initial lock; a `STATE_WRITING` spin refreshes the shared recovery timestamp.
    fn try_extract_message_into(&self, spin_start_ticks: i64, buf: &mut Vec<u8>) -> bool {
        let header = self.res.header();
        if header.is_empty() {
            return false;
        }
        let read_offset = header.read_offset.load(Ordering::Acquire);
        let write_offset = header.write_offset.load(Ordering::Acquire);
        let ring = self.res.ring();
        let Some(msg) = self.message_header_or_drain(&ring, header, read_offset, write_offset)
        else {
            return false;
        };
        if self
            .claim_ready_message(header, msg, write_offset, spin_start_ticks)
            .is_none()
        {
            return false;
        }
        let Some(padded_len) =
            self.read_valid_message_into(ring, header, msg, read_offset, write_offset, buf)
        else {
            return false;
        };
        ring.clear(read_offset, padded_len as usize);
        self.advance_read_offset(header, read_offset, padded_len);
        true
    }

    /// Returns the slot header at `read_offset`, or drains corrupted queue state.
    fn message_header_or_drain<'a>(
        &self,
        ring: &'a RingView,
        header: &QueueHeader,
        read_offset: i64,
        write_offset: i64,
    ) -> Option<&'a MessageHeader> {
        // SAFETY: `read_offset` is produced by the publisher after a space check and the wire
        // protocol guarantees a contiguous eight-byte `MessageHeader` at this slot. A `None`
        // return means the slot is misaligned (corrupted publisher); drain past every queued
        // slot rather than dereference a misaligned pointer.
        let msg = unsafe { ring.message_header_at(read_offset) };
        if msg.is_none() {
            self.drain_to_write_offset(header, write_offset);
        }
        msg
    }

    /// Claims a ready message header or drains the queue if the writer appears stuck too long.
    fn claim_ready_message(
        &self,
        header: &QueueHeader,
        msg: &MessageHeader,
        write_offset: i64,
        spin_start_ticks: i64,
    ) -> Option<()> {
        let mut backoff_iter: u32 = 0;
        let mut refreshed_lock = false;
        loop {
            if msg
                .state
                .compare_exchange(
                    STATE_READY,
                    STATE_LOCKED,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_ok()
            {
                break;
            }
            if !refreshed_lock {
                // One recovery-timestamp update for the spin, instead of a lock drop/reacquire
                // between messages. Ready slots never touch this cache line.
                header
                    .read_lock_timestamp
                    .store(utc_now_ticks(), Ordering::SeqCst);
                refreshed_lock = true;
            }
            if utc_now_ticks() - spin_start_ticks > TICKS_FOR_TEN_SECONDS {
                self.drain_to_write_offset(header, write_offset);
                return None;
            }
            Self::wait_for_extract_retry(backoff_iter);
            backoff_iter = backoff_iter.saturating_add(1);
        }
        Some(())
    }

    /// Copies a validated message body from the ring into `buf`.
    ///
    /// Returns the padded slot length when the body was copied. `buf` is left unchanged when
    /// the slot length is rejected.
    fn read_valid_message_into(
        &self,
        ring: RingView,
        header: &QueueHeader,
        msg: &MessageHeader,
        read_offset: i64,
        write_offset: i64,
        buf: &mut Vec<u8>,
    ) -> Option<i64> {
        let body_len = i64::from(msg.body_length);
        let Some(padded_len) = self.valid_padded_len(body_len) else {
            self.drain_to_write_offset(header, write_offset);
            return None;
        };
        let body_offset = read_offset + MESSAGE_BODY_OFFSET;
        ring.read_into(body_offset, body_len as usize, buf);
        Some(padded_len)
    }

    /// Returns a padded slot length when `body_len` is sane for this queue capacity.
    fn valid_padded_len(&self, body_len: i64) -> Option<i64> {
        if body_len < 0 || body_len > self.res.capacity {
            return None;
        }
        let padded = padded_message_length(body_len);
        if padded > self.res.capacity {
            return None;
        }
        Some(padded)
    }

    /// Drains every queued slot after corruption or timeout.
    fn drain_to_write_offset(&self, header: &QueueHeader, write_offset: i64) {
        // Release publishes the drain to the publisher's Acquire load in `available_space`.
        header.read_offset.store(write_offset, Ordering::Release);
    }

    /// Advances the logical read offset after consuming one valid message.
    fn advance_read_offset(&self, header: &QueueHeader, read_offset: i64, padded_len: i64) {
        let new_read = (read_offset + padded_len) % (self.res.capacity * 2);
        // Release publishes freed ring space. The slot bytes were cleared above this store.
        header.read_offset.store(new_read, Ordering::Release);
    }

    /// Performs one retry delay while waiting for a publisher to publish a claimed slot.
    fn wait_for_extract_retry(backoff_iter: u32) {
        if backoff_iter < EXTRACT_SPIN_ITERATIONS {
            std::hint::spin_loop();
        } else if backoff_iter < EXTRACT_SPIN_ITERATIONS.saturating_add(EXTRACT_YIELD_ITERATIONS) {
            std::thread::yield_now();
        } else {
            std::thread::sleep(EXTRACT_PARK_INTERVAL);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU64;

    use tempfile::TempDir;

    use super::*;
    use crate::options::QueueOptions;
    use crate::publisher::Publisher;

    /// Unique queue-name suffix for tests sharing the kernel semaphore namespace.
    static QUEUE_SEQ: AtomicU64 = AtomicU64::new(0);

    /// Builds isolated queue options backed by a temporary directory.
    fn queue_options(prefix: &str, capacity: i64) -> Result<(TempDir, QueueOptions), String> {
        let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
        let name = format!(
            "{prefix}_{}_{}",
            std::process::id(),
            QUEUE_SEQ.fetch_add(1, Ordering::Relaxed)
        );
        let opts = QueueOptions::with_path(&name, dir.path(), capacity)
            .map_err(|e| format!("queue options: {e}"))?;
        Ok((dir, opts))
    }

    /// Overwrites the current slot body length without touching the slot state.
    fn corrupt_message_body_len(subscriber: &Subscriber, body_len: i32) {
        let read_offset = subscriber.res.header().read_offset.load(Ordering::SeqCst);
        let field_offset = std::mem::offset_of!(MessageHeader, body_length) as i64;
        subscriber
            .res
            .ring()
            .write(read_offset + field_offset, &body_len.to_le_bytes());
    }

    #[test]
    fn try_dequeue_into_reuses_buffer_capacity() {
        let (_dir, opts) = queue_options("sub_reuse", 4096).expect("queue options");
        let mut publisher = Publisher::new(opts.clone()).expect("publisher");
        let mut subscriber = Subscriber::new(opts).expect("subscriber");
        let mut buf = Vec::with_capacity(128);
        assert!(publisher.try_enqueue(b"reuse-me"));
        assert!(subscriber.try_dequeue_into(&mut buf));
        assert_eq!(buf, b"reuse-me");
        let capacity = buf.capacity();
        assert!(publisher.try_enqueue(b"again"));
        assert!(subscriber.try_dequeue_into(&mut buf));
        assert_eq!(buf, b"again");
        assert!(buf.capacity() >= capacity);
        assert!(!subscriber.try_dequeue_into(&mut buf));
        assert_eq!(buf, b"again");
    }

    #[test]
    fn try_drain_reusing_preserves_fifo_and_limit() {
        let (_dir, opts) = queue_options("sub_drain", 4096).expect("queue options");
        let mut publisher = Publisher::new(opts.clone()).expect("publisher");
        let mut subscriber = Subscriber::new(opts).expect("subscriber");
        for payload in [b"one".as_slice(), b"two", b"three", b"four"] {
            assert!(publisher.try_enqueue(payload));
        }

        let mut recycled = vec![Vec::with_capacity(32), Vec::with_capacity(32)];
        let mut out = Vec::new();
        assert_eq!(subscriber.try_drain_reusing(&mut recycled, &mut out, 2), 2);
        assert_eq!(out[0], b"one");
        assert_eq!(out[1], b"two");
        assert_eq!(subscriber.try_drain_reusing(&mut recycled, &mut out, 0), 0);
        assert_eq!(out.len(), 2);

        recycled.append(&mut out);
        let recycled_before = recycled.len();
        assert_eq!(subscriber.try_drain_reusing(&mut recycled, &mut out, 8), 2);
        assert_eq!(out[0], b"three");
        assert_eq!(out[1], b"four");
        assert!(recycled.len() < recycled_before);
        assert!(subscriber.try_dequeue().is_none());
    }

    #[test]
    fn try_dequeue_empty_returns_none() {
        let (_dir, opts) = queue_options("sub_empty", 4096).expect("queue options");
        let mut subscriber = Subscriber::new(opts).expect("subscriber");
        assert!(subscriber.try_dequeue().is_none());
    }

    #[test]
    fn dequeue_respects_cancel_when_idle() {
        let (_dir, opts) = queue_options("sub_cancel", 4096).expect("queue options");
        let mut subscriber = Subscriber::new(opts).expect("subscriber");
        let cancel = AtomicBool::new(true);
        assert!(subscriber.dequeue(&cancel).is_empty());
    }

    #[test]
    fn dequeue_after_message_then_cancel() {
        let (_dir, opts) = queue_options("sub_cancel2", 4096).expect("queue options");
        let mut publisher = Publisher::new(opts.clone()).expect("publisher");
        let mut subscriber = Subscriber::new(opts).expect("subscriber");
        assert!(publisher.try_enqueue(b"ping"));
        assert_eq!(
            subscriber.try_dequeue().as_deref(),
            Some(b"ping".as_slice())
        );
        let cancel = AtomicBool::new(true);
        assert!(subscriber.dequeue(&cancel).is_empty());
    }

    #[test]
    fn wait_for_message_timeout_observes_publish() {
        let (_dir, opts) = queue_options("sub_wait", 4096).expect("queue options");
        let mut publisher = Publisher::new(opts.clone()).expect("publisher");
        let mut subscriber = Subscriber::new(opts).expect("subscriber");

        assert!(!subscriber.wait_for_message_timeout(Duration::from_millis(0)));
        assert!(publisher.try_enqueue(b"payload"));
        assert!(subscriber.wait_for_message_timeout(Duration::from_millis(50)));
        assert_eq!(
            subscriber.try_dequeue().as_deref(),
            Some(b"payload".as_slice())
        );
    }

    #[test]
    fn fifo_across_many_messages() {
        let (_dir, opts) = queue_options("sub_fifo", 4096).expect("queue options");
        let mut publisher = Publisher::new(opts.clone()).expect("publisher");
        let mut subscriber = Subscriber::new(opts).expect("subscriber");
        for i in 0u32..30 {
            assert!(publisher.try_enqueue(format!("n{i}").as_bytes()));
        }
        for i in 0u32..30 {
            let expected = format!("n{i}");
            assert_eq!(
                subscriber.try_dequeue().as_deref(),
                Some(expected.as_bytes())
            );
        }
    }

    #[test]
    fn try_dequeue_drains_when_body_length_is_negative() {
        let (_dir, opts) = queue_options("sub_negative_len", 4096).expect("queue options");
        let mut publisher = Publisher::new(opts.clone()).expect("publisher");
        let mut subscriber = Subscriber::new(opts).expect("subscriber");
        assert!(publisher.try_enqueue(b"bad"));
        let write_offset = subscriber.res.header().write_offset.load(Ordering::SeqCst);

        corrupt_message_body_len(&subscriber, -1);

        assert!(subscriber.try_dequeue().is_none());
        assert_eq!(
            subscriber.res.header().read_offset.load(Ordering::SeqCst),
            write_offset
        );
        assert!(subscriber.res.header().is_empty());
    }

    #[test]
    fn try_dequeue_drains_when_padded_body_length_exceeds_capacity() {
        let cap = 24i64;
        let (_dir, opts) = queue_options("sub_padded_len", cap).expect("queue options");
        let mut publisher = Publisher::new(opts.clone()).expect("publisher");
        let mut subscriber = Subscriber::new(opts).expect("subscriber");
        assert!(publisher.try_enqueue(b"x"));
        let write_offset = subscriber.res.header().write_offset.load(Ordering::SeqCst);

        corrupt_message_body_len(&subscriber, cap as i32);

        assert!(subscriber.try_dequeue().is_none());
        assert_eq!(
            subscriber.res.header().read_offset.load(Ordering::SeqCst),
            write_offset
        );
        assert!(subscriber.res.header().is_empty());
    }

    #[test]
    fn utc_now_ticks_is_after_dotnet_unix_epoch() {
        let now = utc_now_ticks();
        assert!(
            now > DOTNET_TICKS_AT_UNIX_EPOCH,
            "utc_now_ticks() = {now} should be past the .NET Unix epoch"
        );
        // Year 3000 in .NET ticks: ~9.4e17. A loose upper bound that fails loudly on a
        // saturating-add or sign-flip regression while leaving room for clock drift.
        const YEAR_3000_DOTNET_TICKS: i64 = 946_708_416_000_000_000;
        assert!(
            now < YEAR_3000_DOTNET_TICKS,
            "utc_now_ticks() = {now} unexpectedly past year 3000 -- saturating-add bug?"
        );
    }

    /// Verifies `Subscriber::new` surfaces an `OpenError` when the backing directory cannot
    /// accept a new file. Unix-only because the equivalent permission denial on Windows is
    /// brittle to set up reliably from a unit test.
    #[cfg(unix)]
    #[test]
    fn subscriber_new_returns_err_for_unwritable_path() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        struct PermsGuard {
            path: std::path::PathBuf,
        }

        impl Drop for PermsGuard {
            fn drop(&mut self) {
                let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o755));
            }
        }

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().to_path_buf();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o500)).expect("chmod 0o500");
        let _guard = PermsGuard { path: path.clone() };

        let opts =
            QueueOptions::with_path("sub_no_perm", &path, 4096).expect("options should validate");
        assert!(
            Subscriber::new(opts).is_err(),
            "expected Subscriber::new to fail when backing dir is read-only"
        );
    }
}
