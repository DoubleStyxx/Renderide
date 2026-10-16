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
7. Check the draft release. Assets are one zip per platform and `SHA256SUMS`. Download them and verify:

   ```bash
   sha256sum -c SHA256SUMS
   gh attestation verify renderide-linux-x86_64-vX.Y.Z.zip --repo DoubleStyxx/Renderide
   gh attestation verify SHA256SUMS --repo DoubleStyxx/Renderide
   ```

8. A person publishes the draft from the GitHub release page. The workflow does not publish it.

## Build RenderideMod.dll locally (CachyOS)

Build `RenderideMod.dll` on the maintainer Linux PC against a local Resonite install. The draft release assets are the platform zips and `SHA256SUMS`.

Install the .NET 10 SDK. `RenderideMod.csproj` targets `net10.0`.

```bash
sudo pacman -S dotnet-sdk-10.0
```

`FrooxEngine.dll` and `Renderite.Shared.dll` are HintPath references `$(ResonitePath)FrooxEngine.dll` and `$(ResonitePath)Renderite.Shared.dll`. `ResonitePath` is an MSBuild property. Pass it as `-p:ResonitePath=`, or export the `ResonitePath` environment variable (MSBuild imports environment variables as properties before it evaluates the project). The value needs a trailing slash. When `ResonitePath` is empty, the csproj sets it to `RenderideMod/Resonite/`, then replaces that when one of these directories exists:

- `C:\Program Files (x86)\Steam\steamapps\common\Resonite\`
- `$HOME/.steam/steam/steamapps/common/Resonite/`

A default CachyOS Steam install matches the `$HOME/.steam/steam/...` path:

```bash
dotnet build RenderideMod/RenderideMod.csproj -c Release
```

For a Resonite install on another path:

```bash
dotnet build RenderideMod/RenderideMod.csproj -c Release -p:ResonitePath="$HOME/Games/Resonite/"
```

Output path: `RenderideMod/bin/Release/net10.0/RenderideMod.dll`. `CopyToMods` defaults to true, so the build copies that DLL to `$(ResonitePath)rml_mods`. If the copy misses, put `RenderideMod.dll` in `rml_mods` under the Resonite install. Launch with the README argument `-LoadAssembly Libraries/ResoniteModLoader.dll`.

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

Place the locally built `RenderideMod.dll` in `rml_mods` under the Resonite install (see above). The README starts mods by passing `-LoadAssembly Libraries/ResoniteModLoader.dll` to the launcher:

```bash
./renderide -LoadAssembly Libraries/ResoniteModLoader.dll
```
