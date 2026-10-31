## Summary

<!-- What changed and why. -->

## Test plan

<!-- How you checked it. -->

## Rendering and parity

A change to drawing, shading, lighting, culling, skinning, or Resonite parity must add or update a headless test render under `crates/renderide-test` that fails without the change. Do not widen a golden tolerance to hide a mismatch. Check one line.

- [ ] This PR changes rendering or parity and updates a golden or test scene that fails without the change.
- [ ] This PR does not change rendering or parity.

## Verification evidence

<!-- pr-body-lint:placeholder verification-evidence
Name the golden or test that fails without this change, and link the CI run.
A blessed golden needs the before/after diff in this pull request. See PILLARS.md.
-->

TODO(pr-body): replace this line with the verification evidence.

## Pillars advanced

<!-- pr-body-lint:placeholder pillars-advanced
Tick or list which pillars this pull request advances (performance, fidelity, parity, experimentation, customizability, reliability), say how, and confirm that no pillar regresses, with evidence links.
`None, maintenance only` is acceptable with a reason. See PILLARS.md.
-->

TODO(pr-body): replace this line with the pillars this pull request advances.
