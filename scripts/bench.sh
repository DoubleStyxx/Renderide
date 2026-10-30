#!/usr/bin/env bash
# Desktop frame benchmark for a real GPU (for example an RTX 5080).
# Thin wrapper around `renderide-bench run --suite frame` (B2 front door).
# Builds release binaries and writes JSON under results/framebench by default.
# Does not force lavapipe. CI pins lavapipe in .github/workflows/frame-bench.yml.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

out="${1:-results/framebench}"
warmup="${WARMUP:-30}"
frames="${FRAMES:-120}"
runs="${RUNS:-3}"
resolution="${RESOLUTION:-1920x1080}"

cargo build --release -p renderide -p renderide-bench -p renderide-framebench

cmd=(
  cargo run --release -p renderide-bench -- run
  --suite frame
  --release
  --output "$out"
  --resolution "$resolution"
  --warmup "$warmup"
  --frames "$frames"
  --runs "$runs"
)

if [[ -n "${THREADS:-}" ]]; then
  export RAYON_NUM_THREADS="$THREADS"
  last=$((THREADS - 1))
  if [[ "$THREADS" -eq 1 ]]; then
    cpuset="0"
  else
    cpuset="0-${last}"
  fi
  exec taskset -c "$cpuset" "${cmd[@]}"
fi

exec "${cmd[@]}"
