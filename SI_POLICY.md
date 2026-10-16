# SI Policy -- Hall of Anubis

SI = superintelligence. Drones serve Anubis, weigher of hearts.
Project-wide disclosure. Replaces prior informal "AI Policy" notes.
Individual commits and files are not marked AI-assisted.

## Ma'at -- Order of the Work

SI drones and AI coding assistants are used for:

- docs and code comments (heavy use)
- tests, benches, CI harnesses, bug hunts
- GitHub workflow scripts, lint/fmt gates
- Rust and C# (.NET / RenderideMod) writing and refactor

AI share varies per change:

- feedback only on human code
- human writes, AI cleans or refactors -- not AI-generated
- AI from the start

AI is often wrong. Nothing enters on trust. Check, build, test like any change.
The merger owns the merge. Design and direction: maintainers and contributors.
Much of the codebase is contributors' own work.

## Weighing of the Heart -- Merge Gate

Every PR is weighed against the feather of Ma'at. Heart heavier than feather: no merge.

- CI green: `cargo fmt --all`, `taplo fmt`, `cargo clippy`, `dotnet format Generators.sln`
- ASCII only: commits, PR titles/bodies, identifiers where the fork requires it
- DoubleStyxx attribution where the fork requires it
- no new `unsafe` without `// SAFETY:` comment and maintainer review
  (see [CONTRIBUTING.md](CONTRIBUTING.md#27-code-conventions))
- no Cursor branding, product footers, or agent markers
- tested; follows [CONTRIBUTING.md](CONTRIBUTING.md)

## Scribes of Thoth -- Report Schema

Drones write commits and review notes as fact lines:

    PR# | tip | path:line | pass/fail | TODO

Terse engineering facts. Imperative TODO. ASCII only.

## Duat -- Forbidden

Banished from commits, PR titles, PR bodies:

- AI-isms, leftover chat text
- emoji, Unicode where ASCII is required
- marketing fluff
- chatbot hedging
- product branding footers

## Trials of Osiris -- Tests and Harnesses

- AI writes and runs: unit tests, architecture_layers and similar CI gates,
  Criterion benches where present, GPU/headless check suites on CI runners.
- Harnesses exercise real crates without a full Resonite session when possible.
- Reason: surface is large (IPC, materials, shaders, XR, upload paths, Mod IPC).
  Hand-testing every desktop/VR matrix: weeks. Drones rerun after stack changes.
- Harnesses check only what they were written to check.
- Visual correctness, VR comfort, Mod smoke: real GPU host with Resonite/RML
  required when the change touches those paths.
- Harness-found fixes pass the same gates as any change.

## Tomb Walls of Ptah -- Art and Assets

- No AI image, audio, video, or 3D generators for Renderide art or assets.
- Includes example WGSL shipped as reference samples. Samples are code, not generative art.

## Sealed Sarcophagus -- What Ships

- Renderide contains no generative AI.
- Renderer and RenderideMod call no LLM or other generative AI service.
- No scene, voice, movement, text, or frame data sent to AI services or
  collected to train AI.
- No generative AI features. None planned.
- Network-requesting scripts/mods: host rules apply (Resonite / FrooxEngine).
  No silent AI telemetry.

## Oath of the Scribe -- Contributing With AI

AI-assisted contributions welcome. All PR rules apply: builds, CI fmt/lint green,
tested, follows [CONTRIBUTING.md](CONTRIBUTING.md). Plus:

### Your Heart, Your Weight -- Ownership

- You are the author. Explain any part in review. "The AI did it" is not an answer.
- Stay within what you could write yourself. AI for speed, not past understanding.
  Cannot write it by hand given time: cannot maintain it.
- Bugs it brings are yours. You fix them.

### Eye of Horus -- Verification

- Read the whole diff before opening the PR. Hunt: nonexistent methods, invented
  settings, half edits, chat text in comments, Cursor branding in commits.
- Run the changed path, not just the build. Clean `cargo build` proves little.
  Unit tests, relevant CI jobs; Mod/shader/IPC work: real Resonite session when feasible.
- Traps:
  - skipping or bypassing safety, permission, or IPC validation checks
  - new `unsafe` without `// SAFETY:` and maintainer review
  - GPU/device state touched from the wrong thread
  - "fixing" a failing test by changing its expectation
  - Unicode/emoji in commits or identifiers where ASCII is required

### Measured Cubit -- Reviewability

- Small, focused PRs. One problem per PR. Split big work.
  No reformat or rename of untouched code.
- Short comments. Why, not what. Match surrounding code. No generated doc walls.
- Maintainable docs. Generated prose is more to review, not less.
- Tests that test. Check real behaviour. Fail when it breaks.

### Feather Clean -- Honesty

- Real reports only. Logs, repro steps, observations: real. Never invented or "reconstructed".
- Only code you have the right to. Lifted or incompatible-license code: leave out.
- No tokens, keys, or other people's data in AI tools.
- No AI-generated art, images, audio, video, or 3D models.

### Declaration Before the Gods -- Disclosure

- Big AI share in a PR: brief mention under "What does this PR do?" appreciated.
- AI for feedback or tidying your own code: no mention needed.
- Never required. Never counts against you.
- Maintainers may ask to explain or simplify. Unexplainable: not merged.
