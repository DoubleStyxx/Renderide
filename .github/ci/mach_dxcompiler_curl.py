#!/usr/bin/env python3
"""PATH shim for mach-dxcompiler-rs 0.1.6 build.rs.

That build script always runs `curl` itself. It has no cache environment
variable and no local-path override. It requests two URLs:

* https://api.github.com/repos/DouglasDwyer/mach-dxcompiler/releases/tags/<tag>
* https://github.com/DouglasDwyer/mach-dxcompiler/releases/download/<tag>/<asset>

A warm file under MACH_DXCOMPILER_CACHE is served in place of those calls.
Anything else is passed to the real curl. A miss retries the real curl and
stores a copy for the next call.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import time

MARKER = "DouglasDwyer/mach-dxcompiler"
ATTEMPTS = 5


def cache_dir() -> str:
    path = os.environ.get("MACH_DXCOMPILER_CACHE", "")
    if not path:
        raise SystemExit("MACH_DXCOMPILER_CACHE is not set")
    os.makedirs(path, exist_ok=True)
    return path


def real_curl() -> str:
    override = os.environ.get("MACH_DXCOMPILER_CURL", "")
    if override:
        return override
    system_root = os.environ.get("SYSTEMROOT", r"C:\Windows")
    windows = os.path.join(system_root, "System32", "curl.exe")
    if os.path.isfile(windows):
        return windows
    found = shutil.which("curl")
    if found:
        return found
    raise SystemExit("real curl was not found")


def url_from(args: list[str]) -> str | None:
    urls = [arg for arg in args if arg.startswith("http://") or arg.startswith("https://")]
    if not urls:
        return None
    return urls[-1]


def output_path(args: list[str]) -> str | None:
    if "-o" in args:
        index = args.index("-o")
        if index + 1 < len(args):
            return args[index + 1]
    for arg in args:
        if arg.startswith("-o") and arg != "-o":
            return arg[2:]
    return None


def cache_name(url: str) -> str:
    if "/releases/tags/" in url:
        return "release.json"
    return url.rstrip("/").split("/")[-1]


def serve(path: str, dest: str | None) -> int:
    print(f"mach-dxcompiler cache hit: {os.path.basename(path)}", file=sys.stderr)
    if dest:
        shutil.copyfile(path, dest)
    else:
        with open(path, "rb") as handle:
            sys.stdout.buffer.write(handle.read())
    return 0


def remember(url: str, dest: str | None, stdout: bytes) -> None:
    path = os.path.join(cache_dir(), cache_name(url))
    if dest and os.path.isfile(dest):
        shutil.copyfile(dest, path)
        return
    if stdout:
        with open(path, "wb") as handle:
            handle.write(stdout)


def delegate(args: list[str], url: str | None) -> int:
    last = 1
    for attempt in range(1, ATTEMPTS + 1):
        if url and MARKER in url:
            print(
                f"mach-dxcompiler cache miss: {cache_name(url)} attempt {attempt}",
                file=sys.stderr,
            )
        completed = subprocess.run(
            [real_curl(), *args],
            check=False,
            stdout=subprocess.PIPE if url and MARKER in url and output_path(args) is None else None,
        )
        last = completed.returncode
        if last == 0:
            if url and MARKER in url:
                stdout = completed.stdout or b""
                if output_path(args) is None:
                    sys.stdout.buffer.write(stdout)
                remember(url, output_path(args), stdout)
            return 0
        if attempt < ATTEMPTS and url and MARKER in url:
            time.sleep(min(5 * attempt, 20))
    return last


def main() -> int:
    args = sys.argv[1:]
    url = url_from(args)
    if url and MARKER in url:
        cached = os.path.join(cache_dir(), cache_name(url))
        if os.path.isfile(cached) and os.path.getsize(cached) > 0:
            return serve(cached, output_path(args))
    return delegate(args, url)


if __name__ == "__main__":
    raise SystemExit(main())
