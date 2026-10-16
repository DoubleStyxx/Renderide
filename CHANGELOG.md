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

### Changed

- IPC list packing, plus cached motion-blur and GTAO depth-prefilter bind groups (#1)
- Clustered-light, GTAO, and motion-vector shaders skip repeated ALU, including invalid motion-vector draws (#9)
- IPC dequeue and draw preparation reuse buffers (#12)
- CPU cull plans are reused per render space and draw lists batch through compact keys (#14)
- IPC unsafe is limited to one operation per block, with clippy denies on undocumented unsafe (#19)
- SI policy wording rewritten; the rules are unchanged (#23)

### Fixed

- Headless captures wait past flat frames before image compare (#22)

[Unreleased]: https://github.com/DoubleStyxx/Renderide/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/DoubleStyxx/Renderide/compare/949a13c64f6bb5ecc83abc1b3abab254571eaeb1...v0.1.1
