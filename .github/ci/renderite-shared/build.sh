#!/usr/bin/env bash
# Build Renderite.Shared.dll from Yellow-Dog-Man/Renderite.
# Source: https://github.com/Yellow-Dog-Man/Renderite
# Commit: 7ccfd2ee7c28648cb2e46dd4f52f51613a848071
# License: MIT, Copyright (c) 2026 Yellow Dog Man Studios s.r.o.
# The DLL is a build output. Do not commit it.
set -euo pipefail

expected="7ccfd2ee7c28648cb2e46dd4f52f51613a848071"
if [[ ! "$expected" =~ ^[0-9a-f]{40}$ ]]; then
  echo "Renderite pin must be a full 40-character commit SHA, not a short SHA or branch." >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
src="${1:-${RUNNER_TEMP:-/tmp}/yellow-dog-man-renderite}"
lock="${repo_root}/.github/ci/renderite-shared/packages.lock.json"
props="${repo_root}/.github/ci/renderite-shared/Directory.Build.props"
csproj="${src}/Renderite.Shared/Renderite.Shared.csproj"
out="${src}/artifacts/Release"

if [[ ! -f "$lock" ]]; then
  echo "Missing NuGet lock file: ${lock}" >&2
  exit 1
fi

rm -rf "$src"
mkdir -p "$src"
git -c init.defaultBranch=master init "$src"
git -C "$src" remote add origin https://github.com/Yellow-Dog-Man/Renderite.git
git -C "$src" fetch --depth 1 origin "$expected"
git -C "$src" checkout --quiet --detach FETCH_HEAD

actual="$(git -C "$src" rev-parse HEAD)"
echo "Renderite checkout SHA: ${actual}"
echo "Expected Renderite SHA: ${expected}"
if [[ "$actual" != "$expected" ]]; then
  echo "Renderite SHA mismatch: got ${actual}, expected ${expected}" >&2
  exit 1
fi
git -C "$src" log -1 --format='Renderite commit: %H %s'

if [[ ! -f "${src}/LICENSE.txt" ]] || ! grep -q "MIT License" "${src}/LICENSE.txt"; then
  echo "Expected the MIT LICENSE.txt from Yellow-Dog-Man/Renderite at ${expected}" >&2
  exit 1
fi
echo "Renderite license: MIT, Copyright (c) 2026 Yellow Dog Man Studios s.r.o. (${src}/LICENSE.txt)"
echo "Renderite source: https://github.com/Yellow-Dog-Man/Renderite"

cp "$props" "${src}/Directory.Build.props"

dotnet restore "$csproj" \
  --locked-mode \
  -p:RestorePackagesWithLockFile=true \
  -p:NuGetLockFilePath="$lock"

# Publish, not a bare build: the roundtrip loader resolves sibling managed
# dependencies (Elements.Data, Enums.NET, and the rest of the locked graph)
# from the DLL's directory. `dotnet build -o` does not copy those assemblies.
dotnet publish "$csproj" \
  --configuration Release \
  --no-restore \
  -p:NuGetLockFilePath="$lock" \
  -o "$out"

dll="${out}/Renderite.Shared.dll"
if [[ ! -s "$dll" ]]; then
  echo "Renderite.Shared.dll was not produced at ${dll}" >&2
  exit 1
fi

dll_env="$dll"
if [[ "${RUNNER_OS:-}" == "Windows" ]] && command -v cygpath >/dev/null 2>&1; then
  dll_env="$(cygpath -m "$dll")"
fi

echo "Built Renderite.Shared.dll: ${dll_env}"
if [[ -n "${GITHUB_ENV:-}" ]]; then
  echo "RENDERITE_SHARED_DLL=${dll_env}" >> "$GITHUB_ENV"
fi
