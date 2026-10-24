## Summary

<!-- What changed and why. -->

## Test plan

<!-- How you checked it. -->

## Rendering and parity

A change to drawing, shading, lighting, culling, skinning, or Resonite parity must add or update a headless test render under `crates/renderide-test` that fails without the change. Do not widen a golden tolerance to hide a mismatch. Check one line.

- [ ] This PR changes rendering or parity and updates a golden or test scene that fails without the change.
- [ ] This PR does not change rendering or parity.
