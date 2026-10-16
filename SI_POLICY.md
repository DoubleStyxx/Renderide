# SI Policy

Renderide is built with help from AI tools under an SI (systems-intelligence) workflow. We'd rather say that up front than have people guess, so this covers how AI is used to make Renderide, what actually ships in it, and what we ask of contributors who use AI.

This document replaces any prior informal "AI Policy" notes. It is the project-wide disclosure; we do not mark individual commits or files as AI-assisted.

## How Renderide is made

We use AI coding assistants (including Cursor agents and specialist SI drones coordinated by maintainers) for:

- documentation and code comments, which is where we lean on them often
- writing and running tests, benches, and CI harnesses, and tracking down bugs
- sorting out GitHub workflow scripts and lint/fmt gates
- helping write and refactor Rust and C# (.NET / RenderideMod) code

How much AI is involved varies a lot from change to change. Sometimes it is only used for feedback on code a person wrote. Sometimes a person writes the code first and AI helps clean it up or refactor it, which does not make that code AI-generated. And some code is written with AI from the start.

AI gets code wrong a lot, so nothing it produces goes in on trust. It gets checked, built, and tested like any other change, and whoever merges it owns it. The design and direction come from the maintainers and contributors, and a lot of the codebase is contributors' own work.

### SI voice for automated agents

Automated agents that open or update PRs on this fork should keep commits and review notes in SI form: terse engineering facts (tip SHA, path:line, pass/fail, imperative TODO), ASCII only, DoubleStyxx attribution where the fork requires it. No marketing fluff, emoji, product branding footers, or chatbot hedging in commit messages or PR titles/bodies.

### Tests and harnesses

A lot of renderer coverage is done with automated tests and headless harnesses that AI helps write and run: unit tests, architecture_layers and similar CI gates, Criterion benches where present, and GPU/headless check suites on CI runners. These exercise real crates without a full Resonite session when possible.

We use AI here because of how much surface there is to cover (IPC, materials, shaders, XR, upload paths, Mod IPC). Hand-testing every matrix on desktop and VR would take weeks. Agents can keep writing and rerunning scenarios after stack changes. Harnesses only check what they were written to check: visual correctness, VR comfort, and Mod smoke still need a real GPU host with Resonite/RML when the change touches those paths. Fixes for anything a harness finds go through the same checks as any other change.

## Art and assets

We do not use AI image, audio, video, or 3D generators to make Renderide's art or assets (including example WGSL content that ships as reference samples -- treat samples as code, not generative art).

## What ships in Renderide

Renderide does not include generative AI. The renderer and RenderideMod do not call LLMs or any other generative AI service, and they do not send your scene, voice, movement, text, or frame data to AI services, or collect it to train AI. There are no generative AI features in Renderide and none are planned.

Scripts or mods that make network requests are subject to the host application's rules (Resonite / FrooxEngine); do not add silent AI telemetry.

## Using AI in your contributions

AI-assisted contributions are welcome. The usual PR rules still apply: it builds, it is formatted (`cargo fmt`, taplo where TOML changes, `dotnet` format/build for Mod), Clippy and CI are green, you have tested it, and you follow [CONTRIBUTING.md](CONTRIBUTING.md). On top of that:

### You own it

- **You're the author.** Whatever the AI wrote, you're the one submitting it. Understand it well enough to explain any part of it in review. If a reviewer asks why something works the way it does, "the AI did it" is not an answer.
- **Stay within what you could write yourself.** Use AI to go faster, not to go past what you understand. If you could not have written it by hand given enough time, you will not be able to maintain it either.
- **Bugs it brings in are yours.** If it breaks something, you will be asked to fix it.

### Check it properly

- **Read the whole diff before opening the PR.** AI code often looks right and is not: calls to methods that do not exist, made-up settings, half-finished edits, leftover chat text in comments, Cursor branding in commit messages.
- **Run it where it matters, not just the build.** A clean `cargo build` proves very little. Exercise the path you changed (unit tests, relevant CI jobs, and for Mod/shader/IPC work a real Resonite session when feasible).
- **Watch for the usual traps:**
  - skipping or working around safety, permission, or IPC validation checks
  - introducing new `unsafe` without an allowlist path and SAFETY comments (see SI unsafe/clippy policy)
  - touching GPU/device state from the wrong thread
  - "fixing" a failing test by changing what it expects
  - Unicode/emoji in commit messages or identifiers where the fork requires ASCII

### Keep it reviewable

- **Small, focused PRs.** AI makes it easy to change 50 files at once. Keep to one problem per PR and split big work up. Do not reformat or rename code you did not need to touch.
- **Short comments.** Say why, not what, and match the code around it. No walls of generated doc comments.
- **Docs people can maintain.** A page of generated prose is more to review and keep up to date, not less.
- **Tests that test something.** AI is good at writing tests, and they are welcome, but a test has to check real behaviour and fail when that behaviour breaks.

### Keep it honest and clean

- **Real reports only.** AI can help write up a bug report or feature request, but the logs, repro steps, and what you saw have to be real, never made up or "reconstructed".
- **Only submit code you have the right to.** AI tools can reproduce code from other projects, including licenses that cannot be used here. If something looks lifted, leave it out.
- **Keep private stuff out of AI tools.** Do not paste tokens, keys, or other people's data into them.
- **No AI-generated assets.** Do not submit AI-generated art, images, audio, video, or 3D models.

### Saying you used AI

If a big part of a PR was written with AI, a quick mention under "What does this PR do?" is appreciated. Using AI for feedback, or to tidy up code you wrote yourself, does not need a mention. It is never required and will not count against you. Maintainers may ask you to explain or simplify parts of a PR, and if it cannot be explained, it will not be merged.
