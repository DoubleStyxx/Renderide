//! Multi-thread contention bench for the `parking_lot` locks the renderer uses.
//!
//! `parking_lot` 0.13 removed automatic eventual fairness and changed wake-up timing, which CI
//! cannot observe. This binary links 0.12.5 and 0.13 side by side and runs identical workloads
//! that mirror the renderer's lock sites, then prints throughput and wait-latency tails.
//!
//! Workloads (each runs for `--millis` per sample):
//!
//! - `gate`: the GPU queue access gate. One "driver" thread takes a `Mutex<()>` for a short
//!   submit-like critical section; `N-1` "uploader" threads re-lock in a tight loop, half of them
//!   via `try_lock` (the non-blocking texture-upload path). The driver's worst-case wait is the
//!   starvation signal that eventual fairness used to bound.
//! - `hot_mutex`: `N` threads incrementing a shared counter under one `Mutex` with no think time.
//! - `rwlock`: `N-1` readers and one writer that writes every ~50 us (registry-style read-mostly data).
//! - `condvar`: producer/consumer round trips through `Mutex` plus `Condvar` (completion latency).
//!
//! Usage: `cargo run --release -p renderide-lockbench -- [--threads 2,4,8] [--millis 1000] [--samples 5]`.
#![expect(clippy::print_stdout, reason = "this binary's output is its report")]
#![expect(clippy::print_stderr, reason = "usage errors go to stderr")]

use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Log2-bucketed latency histogram, 4 sub-buckets per octave, nanosecond resolution.
#[derive(Clone)]
struct Hist {
    counts: Vec<u64>,
    max_ns: u64,
    total: u64,
}

impl Hist {
    fn new() -> Self {
        Self {
            counts: vec![0; 64 * 4],
            max_ns: 0,
            total: 0,
        }
    }

    fn bucket(ns: u64) -> usize {
        let ns = ns.max(1);
        let octave = 63 - ns.leading_zeros() as usize;
        let sub = if octave >= 2 {
            ((ns >> (octave - 2)) & 3) as usize
        } else {
            0
        };
        octave * 4 + sub
    }

    fn upper_ns(bucket: usize) -> u64 {
        let octave = bucket / 4;
        let sub = (bucket % 4) as u64;
        if octave < 2 {
            return 1u64 << (octave + 1);
        }
        (1u64 << octave) + ((sub + 1) << (octave - 2))
    }

    fn record(&mut self, ns: u64) {
        if let Some(slot) = self.counts.get_mut(Self::bucket(ns)) {
            *slot += 1;
        }
        self.max_ns = self.max_ns.max(ns);
        self.total += 1;
    }

    fn merge(&mut self, other: &Self) {
        for (a, b) in self.counts.iter_mut().zip(&other.counts) {
            *a += *b;
        }
        self.max_ns = self.max_ns.max(other.max_ns);
        self.total += other.total;
    }

    fn quantile(&self, q: f64) -> u64 {
        if self.total == 0 {
            return 0;
        }
        let target = ((self.total as f64) * q).ceil() as u64;
        let mut seen = 0;
        for (i, c) in self.counts.iter().enumerate() {
            seen += *c;
            if seen >= target {
                return Self::upper_ns(i).min(self.max_ns.max(1));
            }
        }
        self.max_ns
    }
}

/// Result of one workload sample.
#[derive(Clone, Default)]
struct Sample {
    /// Total lock acquisitions per second across all threads.
    ops_per_s: f64,
    /// Wait-latency p50 / p99 / p99.9 / max in ns for the watched role.
    p50: u64,
    p99: u64,
    p999: u64,
    max: u64,
    /// Smallest per-thread share divided by the largest (1.0 is perfectly even).
    fairness: f64,
}

fn spin_ns(ns: u64) {
    let start = Instant::now();
    let mut x = 0u64;
    while (start.elapsed().as_nanos() as u64) < ns {
        x = x.wrapping_add(1);
        black_box(x);
    }
}

fn finish(hist: &Hist, ops: &[u64], secs: f64) -> Sample {
    let total: u64 = ops.iter().sum();
    let min = ops.iter().copied().min().unwrap_or(0) as f64;
    let max = ops.iter().copied().max().unwrap_or(0) as f64;
    Sample {
        ops_per_s: total as f64 / secs,
        p50: hist.quantile(0.50),
        p99: hist.quantile(0.99),
        p999: hist.quantile(0.999),
        max: hist.max_ns,
        fairness: if max > 0.0 { min / max } else { 0.0 },
    }
}

/// Expands every workload for one `parking_lot` major.
macro_rules! suite {
    ($modname:ident, $pl:ident) => {
        mod $modname {
            use super::*;
            use $pl::{Condvar, Mutex, RwLock};

            pub fn gate(threads: usize, dur: Duration) -> Sample {
                let gate = Arc::new(Mutex::new(()));
                let stop = Arc::new(AtomicBool::new(false));
                let mut handles = Vec::new();
                for t in 0..threads {
                    let gate = Arc::clone(&gate);
                    let stop = Arc::clone(&stop);
                    handles.push(thread::spawn(move || {
                        let mut hist = Hist::new();
                        let mut ops = 0u64;
                        while !stop.load(Ordering::Relaxed) {
                            if t == 0 {
                                // Driver: submit-like critical section, then a short gap.
                                let start = Instant::now();
                                let guard = gate.lock();
                                hist.record(start.elapsed().as_nanos() as u64);
                                spin_ns(20_000);
                                drop(guard);
                                spin_ns(10_000);
                            } else if t % 2 == 1 {
                                // Blocking uploader: tight re-lock.
                                let guard = gate.lock();
                                spin_ns(5_000);
                                drop(guard);
                            } else if let Some(guard) = gate.try_lock() {
                                // Non-blocking uploader: skip when busy.
                                spin_ns(5_000);
                                drop(guard);
                            } else {
                                thread::yield_now();
                            }
                            ops += 1;
                        }
                        (hist, ops)
                    }));
                }
                thread::sleep(dur);
                stop.store(true, Ordering::Relaxed);
                let mut driver = Hist::new();
                let mut ops = Vec::new();
                for (i, h) in handles.into_iter().enumerate() {
                    if let Ok((hist, n)) = h.join() {
                        if i == 0 {
                            driver.merge(&hist);
                        }
                        ops.push(n);
                    }
                }
                finish(&driver, &ops, dur.as_secs_f64())
            }

            pub fn hot_mutex(threads: usize, dur: Duration) -> Sample {
                let counter = Arc::new(Mutex::new(0u64));
                let stop = Arc::new(AtomicBool::new(false));
                let handles: Vec<_> = (0..threads)
                    .map(|_| {
                        let counter = Arc::clone(&counter);
                        let stop = Arc::clone(&stop);
                        thread::spawn(move || {
                            let mut hist = Hist::new();
                            let mut ops = 0u64;
                            while !stop.load(Ordering::Relaxed) {
                                let start = Instant::now();
                                let mut guard = counter.lock();
                                hist.record(start.elapsed().as_nanos() as u64);
                                *guard += 1;
                                drop(guard);
                                ops += 1;
                            }
                            (hist, ops)
                        })
                    })
                    .collect();
                thread::sleep(dur);
                stop.store(true, Ordering::Relaxed);
                let mut hist = Hist::new();
                let mut ops = Vec::new();
                for h in handles {
                    if let Ok((th, n)) = h.join() {
                        hist.merge(&th);
                        ops.push(n);
                    }
                }
                finish(&hist, &ops, dur.as_secs_f64())
            }

            pub fn rwlock(threads: usize, dur: Duration) -> Sample {
                let data = Arc::new(RwLock::new(vec![1u64; 64]));
                let stop = Arc::new(AtomicBool::new(false));
                let handles: Vec<_> = (0..threads)
                    .map(|t| {
                        let data = Arc::clone(&data);
                        let stop = Arc::clone(&stop);
                        thread::spawn(move || {
                            let mut hist = Hist::new();
                            let mut ops = 0u64;
                            while !stop.load(Ordering::Relaxed) {
                                if t == 0 {
                                    // Writer: watched role.
                                    spin_ns(50_000);
                                    let start = Instant::now();
                                    let mut guard = data.write();
                                    hist.record(start.elapsed().as_nanos() as u64);
                                    if let Some(first) = guard.first_mut() {
                                        *first += 1;
                                    }
                                    drop(guard);
                                } else {
                                    let guard = data.read();
                                    black_box(guard.iter().sum::<u64>());
                                    drop(guard);
                                }
                                ops += 1;
                            }
                            (hist, ops)
                        })
                    })
                    .collect();
                thread::sleep(dur);
                stop.store(true, Ordering::Relaxed);
                let mut writer = Hist::new();
                let mut ops = Vec::new();
                for (i, h) in handles.into_iter().enumerate() {
                    if let Ok((th, n)) = h.join() {
                        if i == 0 {
                            writer.merge(&th);
                        }
                        ops.push(n);
                    }
                }
                let readers: Vec<u64> = ops.iter().skip(1).copied().collect();
                let mut s = finish(&writer, &readers, dur.as_secs_f64());
                s.ops_per_s = ops.iter().sum::<u64>() as f64 / dur.as_secs_f64();
                s
            }

            pub fn condvar(_threads: usize, dur: Duration) -> Sample {
                // Ping-pong a token between two threads through Mutex + Condvar.
                let state = Arc::new((Mutex::new(0u64), Condvar::new()));
                let stop = Arc::new(AtomicBool::new(false));
                let echo = {
                    let state = Arc::clone(&state);
                    let stop = Arc::clone(&stop);
                    thread::spawn(move || {
                        let (lock, cv) = &*state;
                        let mut guard = lock.lock();
                        while !stop.load(Ordering::Relaxed) {
                            if *guard % 2 == 1 {
                                *guard += 1;
                                cv.notify_all();
                            } else {
                                cv.wait_for(&mut guard, Duration::from_millis(5));
                            }
                        }
                        drop(guard);
                    })
                };
                let mut hist = Hist::new();
                let mut trips = 0u64;
                let begin = Instant::now();
                while begin.elapsed() < dur {
                    let start = Instant::now();
                    let (lock, cv) = &*state;
                    let mut guard = lock.lock();
                    *guard += 1;
                    cv.notify_all();
                    while *guard % 2 == 1 {
                        cv.wait_for(&mut guard, Duration::from_millis(5));
                    }
                    drop(guard);
                    hist.record(start.elapsed().as_nanos() as u64);
                    trips += 1;
                }
                stop.store(true, Ordering::Relaxed);
                state.1.notify_all();
                drop(echo.join());
                finish(&hist, &[trips], dur.as_secs_f64())
            }
        }
    };
}

suite!(v012, parking_lot_012);
suite!(v013, parking_lot_013);

type Workload = fn(usize, Duration) -> Sample;

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(0.0)
}

fn med_sample(samples: &[Sample]) -> Sample {
    let pick = |f: fn(&Sample) -> f64| median(samples.iter().map(f).collect());
    Sample {
        ops_per_s: pick(|s| s.ops_per_s),
        p50: pick(|s| s.p50 as f64) as u64,
        p99: pick(|s| s.p99 as f64) as u64,
        p999: pick(|s| s.p999 as f64) as u64,
        max: pick(|s| s.max as f64) as u64,
        fairness: pick(|s| s.fairness),
    }
}

fn main() {
    let mut threads = vec![2usize, 4, 8];
    let mut millis = 1000u64;
    let mut samples = 5usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match (arg.as_str(), args.next()) {
            ("--threads", Some(v)) => {
                threads = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            }
            ("--millis", Some(v)) => millis = v.parse().unwrap_or(millis),
            ("--samples", Some(v)) => samples = v.parse().unwrap_or(samples),
            _ => {
                eprintln!(
                    "usage: renderide-lockbench [--threads 2,4,8] [--millis 1000] [--samples 5]"
                );
                return;
            }
        }
    }
    let dur = Duration::from_millis(millis);
    let workloads: [(&str, &str, Workload, Workload); 4] = [
        ("gate", "driver wait", v012::gate, v013::gate),
        (
            "hot_mutex",
            "lock wait (all)",
            v012::hot_mutex,
            v013::hot_mutex,
        ),
        ("rwlock", "writer wait", v012::rwlock, v013::rwlock),
        ("condvar", "round trip", v012::condvar, v013::condvar),
    ];
    println!(
        "parking_lot 0.12.5 vs 0.13 | threads {threads:?} | {millis} ms x {samples} samples (median), order alternates\n"
    );
    println!(
        "| workload | thr | ver | ops/s | p50 ns | p99 ns | p99.9 ns | max ns | fairness min/max |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for (name, _what, w12, w13) in workloads {
        for &n in &threads {
            if name == "condvar" && n != threads[0] {
                continue;
            }
            let mut a = Vec::new();
            let mut b = Vec::new();
            for i in 0..samples {
                if i % 2 == 0 {
                    a.push(w12(n, dur));
                    b.push(w13(n, dur));
                } else {
                    b.push(w13(n, dur));
                    a.push(w12(n, dur));
                }
            }
            for (ver, s) in [("0.12.5", med_sample(&a)), ("0.13.0", med_sample(&b))] {
                println!(
                    "| {name} | {n} | {ver} | {:.0} | {} | {} | {} | {} | {:.2} |",
                    s.ops_per_s, s.p50, s.p99, s.p999, s.max, s.fairness
                );
            }
        }
    }
}
