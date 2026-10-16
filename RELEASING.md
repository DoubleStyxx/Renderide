# Releasing

Tag releases are draft GitHub releases created by `.github/workflows/release.yml` on a `v*` tag. Nightly zips stay on `.github/workflows/nightly.yml`. Do not publish, merge, or tag from a pull request that only updates this document.

## Steps

1. Update `CHANGELOG.md`. Move the `## [Unreleased]` notes into a new `## [X.Y.Z] - YYYY-MM-DD` section and leave `## [Unreleased]` empty. Point the compare links at the bottom at `vX.Y.Z`.
2. Set the same `X.Y.Z` in `crates/renderide/Cargo.toml` (`version`) and `RenderideMod/RenderideMod.csproj` (`<Version>`). The tag workflow fails unless the tag is `v` plus the crate version.
3. Open a pull request.
4. Wait until CI is green on that pull request and on `master` after it merges.
5. GO gate: do not tag until a GO from DoubleStyxx, relayed by Kairo, is recorded.
6. On `master`, tag the release commit and push the tag:

   ```bash
   git checkout master
   git pull origin master
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

   A `-` in the tag (for example `vX.Y.Z-rc.1`) marks the release as a prerelease. `make_latest` stays false. The workflow leaves the release as a draft.
7. Check the draft release. It should contain one zip per platform, `RenderideMod.dll`, and `SHA256SUMS`. Download the assets and verify them:

   ```bash
   sha256sum -c SHA256SUMS
   gh attestation verify renderide-linux-x86_64-vX.Y.Z.zip --repo DoubleStyxx/Renderide
   gh attestation verify RenderideMod.dll --repo DoubleStyxx/Renderide
   gh attestation verify SHA256SUMS --repo DoubleStyxx/Renderide
   ```

   `RenderideMod.dll` is built only when `FrooxEngine.dll` and `Renderite.Shared.dll` are on the runner. Those assemblies ship with Resonite and are not in this repository. The job fails closed instead of uploading a stand-in. Put both DLLs in the directory named by the `RESONITE_REF_DIR` repository variable, or in `RenderideMod/Resonite/`, before tagging if the draft must contain the mod.
8. A person publishes the draft from the GitHub release page. The workflow does not publish it.

## Linux install (CachyOS, NVIDIA RTX 50-series)

Install the nvidia-open driver and the `vulkan-icd-loader` package:

```bash
sudo pacman -S nvidia-open vulkan-icd-loader
```

Download the Linux zip for this machine (`linux-x86_64` or `linux-aarch64`) and `SHA256SUMS` from the published release. From the directory that contains those files:

```bash
sha256sum -c SHA256SUMS
unzip renderide-linux-x86_64-vX.Y.Z.zip
```

The zip contains `renderide`, `renderide-renderer`, `shaders`, and `xr`. Linux uses the system OpenXR loader.

Place `RenderideMod.dll` in the game mods directory. The README starts mods by passing `-LoadAssembly Libraries/ResoniteModLoader.dll` to the launcher. ResoniteModLoader reads `rml_mods` under the Resonite install (the folder `RenderideMod.csproj` copies into on a local build):

```bash
./renderide -LoadAssembly Libraries/ResoniteModLoader.dll
```
