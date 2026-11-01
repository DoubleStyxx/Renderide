#!/usr/bin/env python3
"""Fail when CI workflows float the Rust toolchain or disagree with rust-toolchain.toml.

Source of truth is ``rust-toolchain.toml`` at the repo root: ``[toolchain].channel`` must be
an exact ``X.Y.Z`` version (not ``stable`` / ``beta`` / ``nightly``). Every workflow and
composite action under ``.github/`` must install that same version. Floating action refs
(``dtolnay/rust-toolchain@stable`` and friends) and ``toolchain: stable|beta|nightly`` (or
any version other than the pin) are errors.

Run: ``python3 scripts/ci/check_rust_toolchain_pin.py`` from the repo root (exit 0 = ok).
Unit tests: ``python3 -m unittest scripts.ci.test_check_rust_toolchain_pin`` or discover.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

FLOATING_CHANNELS = frozenset({"stable", "beta", "nightly"})

# Action ref that floats the channel (tag name is the channel).
FLOATING_ACTION_REF = re.compile(
    r"(?P<pre>uses:\s*)(?P<action>dtolnay/rust-toolchain)@(?P<ref>stable|beta|nightly)\b",
    re.IGNORECASE,
)

# `toolchain: <value>` in a with: block (YAML simple scalar).
TOOLCHAIN_WITH = re.compile(
    r"(?P<pre>^\s*toolchain:\s*)(?P<val>[\"']?)(?P<channel>[^\s\"'#]+)(?P<q>[\"']?)",
    re.MULTILINE | re.IGNORECASE,
)

# rustup / rustc channel selectors that float.
FLOATING_SHELL = re.compile(
    r"""(?x)
    (?:
        rustup\s+(?:default|toolchain\s+install|install)\s+(?:--profile\s+\S+\s+)?
        | rustc\s+\+
        | cargo\s+\+
    )
    (?P<channel>stable|beta|nightly)\b
    """,
    re.IGNORECASE,
)

CHANNEL_VERSION = re.compile(r"^1\.\d+\.\d+$")


def parse_pin(toml_text: str) -> str:
    """Return the exact ``channel`` from a rust-toolchain.toml body."""
    in_toolchain = False
    for raw in toml_text.splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            in_toolchain = line == "[toolchain]"
            continue
        if not in_toolchain:
            continue
        if line.startswith("channel"):
            _, _, rhs = line.partition("=")
            channel = rhs.strip().strip("\"'")
            if channel in FLOATING_CHANNELS:
                raise ValueError(
                    f"rust-toolchain.toml channel must be an exact version, not {channel!r}"
                )
            if not CHANNEL_VERSION.match(channel):
                raise ValueError(
                    f"rust-toolchain.toml channel must look like 1.xx.y, got {channel!r}"
                )
            return channel
    raise ValueError("rust-toolchain.toml missing [toolchain].channel")


def iter_github_yaml(root: Path) -> list[Path]:
    github = root / ".github"
    out: list[Path] = []
    for sub in ("workflows", "actions"):
        base = github / sub
        if not base.is_dir():
            continue
        out.extend(sorted(p for p in base.rglob("*") if p.suffix in {".yml", ".yaml"}))
    return out


def check_file(path: Path, text: str, pin: str) -> list[str]:
    """Return human-readable error strings for one workflow/action file."""
    rel = path.as_posix()
    errors: list[str] = []
    for m in FLOATING_ACTION_REF.finditer(text):
        errors.append(
            f"{rel}: floating rust-toolchain action ref "
            f"`{m.group('action')}@{m.group('ref')}` (pin is {pin})"
        )
    for m in TOOLCHAIN_WITH.finditer(text):
        channel = m.group("channel")
        if channel in FLOATING_CHANNELS:
            errors.append(
                f"{rel}: floating `toolchain: {channel}` (pin is {pin})"
            )
        elif CHANNEL_VERSION.match(channel) and channel != pin:
            errors.append(
                f"{rel}: `toolchain: {channel}` does not match rust-toolchain.toml pin {pin}"
            )
        elif channel not in FLOATING_CHANNELS and not CHANNEL_VERSION.match(channel):
            # e.g. ${{ }} expressions or unexpected tokens — flag them.
            errors.append(
                f"{rel}: unrecognized `toolchain: {channel}` (expected exact pin {pin})"
            )
    for m in FLOATING_SHELL.finditer(text):
        errors.append(
            f"{rel}: floating shell toolchain `{m.group('channel')}` (pin is {pin})"
        )
    return errors


def check_tree(root: Path) -> list[str]:
    pin_path = root / "rust-toolchain.toml"
    if not pin_path.is_file():
        return ["missing rust-toolchain.toml at repo root"]
    try:
        pin = parse_pin(pin_path.read_text(encoding="utf-8"))
    except ValueError as exc:
        return [str(exc)]
    errors: list[str] = []
    files = iter_github_yaml(root)
    if not files:
        errors.append("no .github/workflows or .github/actions YAML files found")
    for path in files:
        errors.extend(check_file(path, path.read_text(encoding="utf-8"), pin))
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=None,
        help="repo root (default: parent of scripts/ci/)",
    )
    args = parser.parse_args(argv)
    root = args.root
    if root is None:
        root = Path(__file__).resolve().parents[2]
    errors = check_tree(root.resolve())
    if errors:
        print("rust toolchain pin check FAILED:", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        return 1
    pin = parse_pin((root / "rust-toolchain.toml").read_text(encoding="utf-8"))
    print(f"rust toolchain pin OK: {pin}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
