# renderide-lockbench

Multi-thread contention micro-benchmark comparing `parking_lot` 0.12.5 and 0.13 on the lock patterns the
renderer uses (queue gate, hot mutex, `RwLock`, `Condvar` hand-off). It links both majors side by side and
prints a table of ops/s and spread per thread count.

## Standalone package

This crate is **not** a member of the root workspace (`exclude` in the root `Cargo.toml`). It has its own
`Cargo.lock` so the benchmark's `parking_lot` 0.13 / `parking_lot_core` 0.10 / `lock_api` 0.5 never enter the
workspace lockfile. Consequences:

- Run it by manifest path, from the repository root:

  ```sh
  cargo run --release --manifest-path crates/renderide-lockbench/Cargo.toml -- --threads 2,4,8
  ```

  Options: `--threads 2,4,8`, `--millis 1000` (per sample), `--samples 5`.
- CI does not build, lint or test it (`cargo clippy`/`cargo test --workspace` only cover workspace members, and
  `cargo fmt --all` skips it). Check it by hand when you change it:

  ```sh
  cargo fmt --manifest-path crates/renderide-lockbench/Cargo.toml -- --check
  cargo clippy --manifest-path crates/renderide-lockbench/Cargo.toml --all-targets --locked -- -D warnings -D clippy::all
  ```

  (`taplo fmt --check` in CI does cover its `Cargo.toml`.)
- It cannot use `[lints] workspace = true`; `Cargo.toml` carries a copy of the root `[workspace.lints.*]`
  tables. Keep them in sync when the root lints change.
- The root `[profile.*]` settings do not apply to it; it builds with Cargo's default release profile.
