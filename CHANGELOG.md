# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - proposed

### Added

- Criterion CPU microbenchmarks for matrix conversion, AABB culling, world-matrix rebuilds, and buffer free-list reuse (#11)
- Custom WGSL shader loading through RenderideMod on the existing ShaderUpload command (#15)
- SI policy document linked from the README and contributing guide (#20)
- Tests for custom shader upload routing when the asset id is negative (#21)
- Custom WGSL material properties mapped from shader bindings (#24)
- CPU benches for material uniform updates and bind-group churn (#25)
- CPU benches for the keyword variant cache (#30)
- Headless frame benchmarks with a relative lavapipe gate (#35)
- Dev-only drone control API (#36)
- Exotic custom WGSL examples and the dynvar uniform contract (#39)

### Changed

- IPC list packing, plus cached motion-blur and GTAO depth-prefilter bind groups (#1)
- Clustered-light, GTAO, and motion-vector shaders skip repeated ALU, including invalid motion-vector draws (#9)
- IPC dequeue and draw preparation reuse buffers (#12)
- CPU cull plans are reused per render space and draw lists batch through compact keys (#14)
- IPC unsafe is limited to one operation per block, with clippy denies on undocumented unsafe (#19)
- SI policy wording rewritten; the rules are unchanged (#23)
- Scratch buffers reused on the renderer frame path (#13)
- Custom material binds keyed by shader generation and texture residency (#26)
- OpenXR events polled while the session is idle (#27)
- World-matrix depth classified in one parent walk (#28)
- Clippy warning denies extended across another lint batch (#29)
- Dirty material uniforms packed from a compiled stem plan (#37)

### Fixed

- Headless captures wait past flat frames before image compare (#22)
- Flaky dev hot reload target mtime check (#31)
- Bootstrapper protocol_handlers tests use unique queue names (#34)

[Unreleased]: https://github.com/DoubleStyxx/Renderide/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/DoubleStyxx/Renderide/compare/949a13c64f6bb5ecc83abc1b3abab254571eaeb1...v0.1.1
