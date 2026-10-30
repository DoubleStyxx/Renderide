#![expect(
    unsafe_code,
    reason = "test-only wrappers call env::set_var and env::remove_var while the caller holds PROCESS_ENV_LOCK"
)]

//! Process-wide lock for test environment changes.
//!
//! Bootstrapper and interprocess tests share this module so every mutation
//! takes the same mutex. On Windows [`std::env::set_var`] is safe. On Unix the
//! remaining hazard is C or FFI `getenv` on another thread, which no Rust lock
//! covers. [`std::env::var`] and [`std::env::var_os`] take the standard
//! library's env lock and are not that hazard. The residual risk is accepted
//! because this module is test-only (`cfg(test)` or the `test-env` feature).

use std::cell::Cell;
use std::ffi::{OsStr, OsString};
use std::marker::PhantomData;
use std::sync::{Mutex, MutexGuard, PoisonError};

const LOCK_MUST_BE_HELD: &str = "PROCESS_ENV_LOCK must be held by this thread \
     while the environment is mutated or read for restore";

thread_local! {
    static HELD_BY_THREAD: Cell<bool> = const { Cell::new(false) };
}

static PROCESS_ENV_LOCK: Mutex<()> = Mutex::new(());

/// Token that holds the process-wide environment lock.
///
/// The guard is `!Sync`, so only the thread that called [`lock`] can use it.
/// Dropping it releases the lock.
#[must_use = "dropping the guard releases the process env lock"]
pub struct EnvLockGuard {
    _guard: MutexGuard<'static, ()>,
    _flag: ThreadFlag,
    // `MutexGuard` is `Sync`. A shared reference on another thread could then
    // call `set_var` while this thread is inside `set_var`. `*const ()` makes
    // this guard `!Sync` so only the locking thread can touch the environment.
    _not_sync: PhantomData<*const ()>,
}

struct ThreadFlag {
    armed: bool,
}

impl ThreadFlag {
    fn arm() -> Self {
        HELD_BY_THREAD.with(|held| {
            held.set(true);
        });
        Self { armed: true }
    }

    fn thread_still_holds(&self) -> bool {
        self.armed && HELD_BY_THREAD.with(Cell::get)
    }
}

impl Drop for ThreadFlag {
    fn drop(&mut self) {
        self.armed = false;
        HELD_BY_THREAD.with(|held| {
            held.set(false);
        });
    }
}

/// Acquires the process-wide environment lock.
///
/// A poisoned lock is recovered so one panicked test does not stick the rest
/// of the process.
pub fn lock() -> EnvLockGuard {
    let guard = PROCESS_ENV_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let flag = ThreadFlag::arm();
    EnvLockGuard {
        _guard: guard,
        _flag: flag,
        _not_sync: PhantomData,
    }
}

/// Debug-checks that this thread holds [`lock`].
///
/// Release builds skip the check. Mutation helpers call this before they touch
/// the environment.
pub fn assert_shared_env_lock_held() {
    debug_assert!(HELD_BY_THREAD.with(Cell::get), "{LOCK_MUST_BE_HELD}");
}

impl EnvLockGuard {
    fn assert_held(&self) {
        debug_assert!(self._flag.thread_still_holds(), "{LOCK_MUST_BE_HELD}");
    }

    /// Sets `key` to `value`.
    ///
    /// # Safety
    ///
    /// `self` must be the [`EnvLockGuard`] from [`lock`], still held on the
    /// thread that acquired it. The guard is `!Send` and `!Sync`, so only that
    /// thread can call this. Bootstrapper and interprocess tests that mutate
    /// the environment take that lock, so they do not run this concurrently
    /// with each other.
    ///
    /// On Windows, [`std::env::set_var`] is safe. On Unix, overlapping this call
    /// with `getenv` is undefined behavior. [`std::env::var`] and
    /// [`std::env::var_os`] already take the standard library's env lock, so
    /// they are not the hazard. C or FFI code that calls `getenv` on another
    /// thread is not covered by any Rust lock. That residual risk is accepted
    /// because this module is test-only.
    ///
    /// # Panics
    ///
    /// Debug builds panic when this thread does not hold the lock.
    pub unsafe fn set_var(&self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) {
        self.assert_held();
        // SAFETY: The caller holds this `EnvLockGuard` (`!Send` and `!Sync`)
        // from `lock`, so this thread holds `PROCESS_ENV_LOCK` until the guard
        // drops. On Windows `std::env::set_var` is safe. On Unix it is undefined
        // behavior if it overlaps `getenv`. `std::env::var` and `var_os` take
        // the standard library's env lock and are not that hazard. C or FFI
        // code calling `getenv` on another thread is not covered by any Rust
        // lock. That residual risk is accepted because this helper is test-only.
        unsafe {
            std::env::set_var(key, value);
        }
    }

    /// Removes `key`.
    ///
    /// # Safety
    ///
    /// `self` must be the [`EnvLockGuard`] from [`lock`], still held on the
    /// thread that acquired it. The guard is `!Send` and `!Sync`, so only that
    /// thread can call this. Bootstrapper and interprocess tests that mutate
    /// the environment take that lock, so they do not run this concurrently
    /// with each other.
    ///
    /// On Windows, [`std::env::remove_var`] is safe. On Unix, overlapping this
    /// call with `getenv` is undefined behavior. [`std::env::var`] and
    /// [`std::env::var_os`] already take the standard library's env lock, so
    /// they are not the hazard. C or FFI code that calls `getenv` on another
    /// thread is not covered by any Rust lock. That residual risk is accepted
    /// because this module is test-only.
    ///
    /// # Panics
    ///
    /// Debug builds panic when this thread does not hold the lock.
    pub unsafe fn remove_var(&self, key: impl AsRef<OsStr>) {
        self.assert_held();
        // SAFETY: The caller holds this `EnvLockGuard` (`!Send` and `!Sync`)
        // from `lock`, so this thread holds `PROCESS_ENV_LOCK` until the guard
        // drops. On Windows `std::env::remove_var` is safe. On Unix it is
        // undefined behavior if it overlaps `getenv`. `std::env::var` and
        // `var_os` take the standard library's env lock and are not that
        // hazard. C or FFI code calling `getenv` on another thread is not
        // covered by any Rust lock. That residual risk is accepted because
        // this helper is test-only.
        unsafe {
            std::env::remove_var(key);
        }
    }

    /// Reads `key` while the lock is held, for capture and restore.
    ///
    /// # Panics
    ///
    /// Debug builds panic when this thread does not hold the lock.
    pub fn var_os(&self, key: impl AsRef<OsStr>) -> Option<OsString> {
        self.assert_held();
        std::env::var_os(key)
    }

    /// Captures `keys` and restores them when the snapshot drops.
    ///
    /// The snapshot borrows this guard, so restore runs before the lock is
    /// released.
    pub fn capture(&self, keys: &[&'static str]) -> EnvSnapshot<'_> {
        self.assert_held();
        EnvSnapshot {
            _guard: self,
            values: keys
                .iter()
                .copied()
                .map(|key| (key, self.var_os(key)))
                .collect(),
        }
    }
}

/// Restores environment variables captured under an [`EnvLockGuard`].
///
/// Drop restores the captured values while the guard is still alive.
#[must_use = "dropping the snapshot restores the captured environment variables"]
pub struct EnvSnapshot<'a> {
    _guard: &'a EnvLockGuard,
    values: Vec<(&'static str, Option<OsString>)>,
}

impl Drop for EnvSnapshot<'_> {
    fn drop(&mut self) {
        for (key, value) in std::mem::take(&mut self.values) {
            if let Some(value) = value {
                // SAFETY: `self._guard` holds `PROCESS_ENV_LOCK` on this thread.
                unsafe {
                    self._guard.set_var(key, value);
                }
            } else {
                // SAFETY: `self._guard` holds `PROCESS_ENV_LOCK` on this thread.
                unsafe {
                    self._guard.remove_var(key);
                }
            }
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::significant_drop_tightening,
    reason = "the assertion and restore snapshot must run while the process env lock is still held"
)]
mod tests {
    use std::ffi::OsStr;

    use super::{LOCK_MUST_BE_HELD, assert_shared_env_lock_held, lock};

    const PROBE: &str = "RENDERIDE_PROCESS_ENV_LOCK_PROBE";

    #[test]
    fn shared_lock_is_held_while_the_environment_is_mutated() {
        let guard = lock();
        assert_shared_env_lock_held();
        let _snapshot = guard.capture(&[PROBE]);
        // SAFETY: `guard` holds `PROCESS_ENV_LOCK` on this thread.
        unsafe {
            guard.set_var(PROBE, "held");
        }
        assert_eq!(guard.var_os(PROBE).as_deref(), Some(OsStr::new("held")));
        // SAFETY: `guard` holds `PROCESS_ENV_LOCK` on this thread.
        unsafe {
            guard.remove_var(PROBE);
        }
        assert!(guard.var_os(PROBE).is_none());
    }

    #[test]
    fn capture_restores_the_value_from_before_the_mutation() {
        let guard = lock();
        let _outer = guard.capture(&[PROBE]);
        // SAFETY: `guard` holds `PROCESS_ENV_LOCK` on this thread.
        unsafe {
            guard.set_var(PROBE, "outer");
        }
        {
            let _inner = guard.capture(&[PROBE]);
            // SAFETY: `guard` holds `PROCESS_ENV_LOCK` on this thread.
            unsafe {
                guard.set_var(PROBE, "inner");
            }
            assert_eq!(guard.var_os(PROBE).as_deref(), Some(OsStr::new("inner")));
        }
        assert_eq!(guard.var_os(PROBE).as_deref(), Some(OsStr::new("outer")));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn mutation_helpers_debug_assert_that_the_shared_lock_is_held() {
        let missing = std::panic::catch_unwind(assert_shared_env_lock_held);
        let payload = missing.expect_err("the shared env lock was not held");
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("non-string panic payload");
        assert!(
            message.contains(LOCK_MUST_BE_HELD),
            "unexpected panic payload: {message}"
        );

        let guard = lock();
        assert_shared_env_lock_held();
        drop(guard);
    }
}
