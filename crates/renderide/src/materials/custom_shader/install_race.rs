//! Concurrent `install` calls must not leave a custom stem without WGSL.
//!
//! Publishing used to drop the registry lock after removing the previous program and before
//! inserting the replacement. Bench tests install one shared stem on several threads, and a
//! lookup in that window panics with `custom stem … has no WGSL`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use super::{install, remove, target_wgsl};

const KEY: &str = "install-race";

/// Counts a worker as finished when its thread returns or panics.
///
/// The watcher waits until this count reaches the worker total. A panic used to skip the
/// increment and leave that wait spinning for the rest of the CI job.
struct WorkerFinished(Arc<AtomicUsize>);

impl Drop for WorkerFinished {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Release);
    }
}

#[test]
fn concurrent_install_keeps_the_stem_wgsl_published() {
    let solid = include_str!("../../../../../RenderideMod/Shaders/example_solid.wgsl");
    let replaced = solid.replace(
        "return rg::retain_globals_additive(mat._Color);",
        "return rg::retain_globals_additive(mat._Color) * 0.5;",
    );
    let seeded = install(KEY, solid).expect("seed");
    let stem = seeded.default_stem;
    assert!(
        target_wgsl(&stem).is_some(),
        "custom stem {stem} has no WGSL"
    );

    let missing_after_install = Arc::new(AtomicUsize::new(0));
    let unpublished = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicUsize::new(0));
    let threads = 8_usize;
    let rounds = 4_usize;

    thread::scope(|scope| {
        let stem_for_watcher = stem.clone();
        let watch_unpublished = Arc::clone(&unpublished);
        let watch_finished = Arc::clone(&finished);
        let watcher = scope.spawn(move || {
            while watch_finished.load(Ordering::Acquire) < threads {
                if target_wgsl(&stem_for_watcher).is_none() {
                    watch_unpublished.fetch_add(1, Ordering::Relaxed);
                }
                thread::yield_now();
            }
        });
        for id in 0..threads {
            let missing_after_install = Arc::clone(&missing_after_install);
            let finished = Arc::clone(&finished);
            let stem = stem.clone();
            let replaced = replaced.clone();
            scope.spawn(move || {
                let _finished = WorkerFinished(finished);
                for round in 0..rounds {
                    let source = if (id + round).is_multiple_of(2) {
                        solid
                    } else {
                        replaced.as_str()
                    };
                    let Ok(installed) = install(KEY, source) else {
                        missing_after_install.fetch_add(1, Ordering::Relaxed);
                        continue;
                    };
                    if installed.default_stem != stem || target_wgsl(&stem).is_none() {
                        missing_after_install.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
        watcher.join().expect("watcher");
    });

    assert_eq!(
        missing_after_install.load(Ordering::Relaxed),
        0,
        "custom stem {stem} has no WGSL"
    );
    assert_eq!(
        unpublished.load(Ordering::Relaxed),
        0,
        "custom stem {stem} was unpublished during install"
    );
    remove(KEY).expect("remove");
    assert!(target_wgsl(&stem).is_none());
}
