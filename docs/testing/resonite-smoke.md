# Resonite interop smoke

Manual checklist for one session of Renderide inside a real Resonite process. Copy the result record at the bottom for every pass. The pass log is the history of those records.

This is a manual supplement. It does not replace the CI goldens (`cargo run -p renderide-test -- check-suite`, images under `crates/renderide-test/goldens/`) or the IPC roundtrip tests (`generators/SharedTypeGenerator.Tests/Roundtrip/CrossLanguageRoundtripTests.cs`, which pack in C# and unpack with the Rust `roundtrip` binary). Hosted CI still omits those roundtrip sources when `Renderite.Shared.dll` is absent. See [PILLARS.md](../../PILLARS.md) under Parity and Reliability.

Run the session on a machine with a physical GPU. Lavapipe and WARP answer the headless suite. They do not answer this checklist.

## Preconditions

Record every line in the result template before marking checklist items.

**OS.** Linux, macOS, and Windows are the launcher targets. The renderer log line `Renderer host system: os=... kernel=...` is the string to copy (`crates/renderide/src/app/bootstrap/logging.rs`).

**GPU and driver.** A wgpu desktop backend: Vulkan, Metal, DirectX 12, or the OpenGL fallback. OpenXR startup uses Vulkan even when the desktop graphics API is something else. After the window or headset comes up, copy the renderer log line that starts `GPU: adapter=` (name, `driver=`, `driver_info=`, `backend=`). Also copy `wgpu adapter selected:`. The CachyOS NVIDIA note in [RELEASING.md](../../RELEASING.md) (`nvidia-open`, `vulkan-icd-loader`) is one documented driver install. Other operating systems use their own driver. The log line is the evidence, not the package name.

**Resonite build.** Steam app `2519830`. The launcher resolves the install in this order: `--resonite-dir` or `-ResoniteDir`, then `RESONITE_DIR`, then Steam (`steamapps/common/Resonite`, including libraries listed in `libraryfolders.vdf`). A valid root contains `Renderite.Host.dll` or `Renderite.Host.exe`. macOS uses the Windows depot and `./target/release/renderide --resonite-dir "$HOME/Games/ResoniteWindows"` as in the [README](../../README.md). Record the version shown in the Resonite session, or the `buildid` field in that library's `appmanifest_2519830.acf` when the file is present. The bootstrapper log line `Bootstrapper paths:` includes `resonite_dir_override=` (`<auto>` when discovery was used).

**Renderide commit.** From the tree you are about to build, record `git rev-parse HEAD`. Build that commit. The renderer log line `Renderer process:` includes `commit_sha8=` and `commit_source=`, baked in at compile time as `RENDERIDE_GIT_COMMIT`. The eight hex digits must match the recorded SHA. A mismatch means the binary is a different commit. The AUR package `renderide-git` is not this procedure.

**Build.** From the repository root:

```bash
git rev-parse HEAD
cargo build --release
./target/release/renderide
```

On Windows the binary is `target/release/renderide.exe`. A published zip is acceptable when you record `commit_sha8` from its log. Linux zips contain `renderide`, `renderide-renderer`, `shaders`, and `xr`. Launch the extracted `./renderide`. Set `RENDERIDE_CONFIG` to pin a config file; otherwise the launcher uses the platform config directory and `Renderide/config.toml`.

**Logs.** The first renderer line `Logging to` is the file for this process. Source builds normally use `logs/renderer/YYYY-MM-DD_HH-MM-SS.log`. `RENDERIDE_LOGS_ROOT` replaces the root. Component directories under that root are `renderer/`, `bootstrapper/`, and `host/`. Installed binaries fall back to `$XDG_STATE_HOME/renderide/logs` or `~/.local/state/renderide/logs` (Linux), `~/Library/Logs/Renderide` (macOS), and `%LOCALAPPDATA%\Renderide\logs` (Windows). Press F7 to show the ImGui UI. Renderer config, Debug tab, shows `Log folder:` and the button `Open log folder`.

**Mod.** Optional for this smoke. Record whether it was loaded. The mod targets `net10.0`. From [RELEASING.md](../../RELEASING.md):

```bash
dotnet build RenderideMod/RenderideMod.csproj -c Release
```

`ResonitePath` needs a trailing slash. When it is empty, the project uses `RenderideMod/Resonite/`, then a default Steam root if one exists (`$HOME/.steam/steam/steamapps/common/Resonite/` or `C:\Program Files (x86)\Steam\steamapps\common\Resonite\`). Another path:

```bash
dotnet build RenderideMod/RenderideMod.csproj -c Release -p:ResonitePath="$HOME/Games/Resonite/"
```

Output is `RenderideMod/bin/Release/net10.0/RenderideMod.dll`. `CopyToMods` defaults to true and copies that DLL to `rml_mods` under the Resonite root. The host only loads it when ResoniteModLoader is already installed and the launcher is started with:

```bash
./target/release/renderide -LoadAssembly Libraries/ResoniteModLoader.dll
```

**Mode and validation.** With no output flag, the launcher asks `Launch Resonite in VR or desktop mode?` (buttons Desktop, VR, Cancel) after the photosensitivity warning (Close, Never Show Again). `-Screen` skips the dialog and starts desktop. `-Device SteamVR` (also `-Device=SteamVR` and `-Device:Screen360`) skips the dialog and starts VR. For the shutdown item, turn on GPU validation before the session: Renderer config, Debug tab, checkbox `GPU validation layers`, then restart, or start with `RENDERIDE_GPU_VALIDATION=1` (`1`, `true`, or `yes`). The same checkbox applies to desktop and OpenXR. Render-graph validation defaults to Warn.

**Video textures.** Stock `cargo build --release` leaves the `video-textures` feature off. Video IPC still allocates a GPU texture, and that texture stays black because nothing decodes. To run checklist item 8, rebuild with `--features video-textures` and install GStreamer (Linux: `libgstreamer1.0-dev`, `libgstreamer-plugins-base1.0-dev`, and `gstreamer1.0-plugins-good`; macOS: `brew install gstreamer`; Windows: the GStreamer MSVC SDK). Record which build you ran.

## Checklist

Mark each item in the result record. `n/a` needs a one-line reason. A pass without the evidence in the next section is not a pass.

1. **Launch and handshake.** Start the binary recorded above. Dismiss the photosensitivity warning. Choose Desktop or VR, or pass `-Screen` / `-Device SteamVR`. Pass: a window or headset shows a frame, and the renderer log contains `IPC connected (Primary/Background)` and `RendererInitData received after`. Fail: `IPC connect failed`, `Timed out waiting for RendererInitData from host`, `Fatal IPC error while waiting for RendererInitData`, or `Standalone mode (no -QueueName/-QueueCapacity` when a host session was the goal. `[ ] pass  [ ] fail`

2. **Local home world loads.** Stay on the world Resonite opens when the host finishes startup. Record the world name shown in the session. Pass: world geometry is visible and the session takes input. Fail: black view, a load that never finishes, or a crash. `[ ] pass  [ ] fail`

3. **Avatar and skinned meshes.** Look at the avatar worn in that world and move. Pass: the skinned mesh follows the pose. Fail: missing avatar, stuck bind pose, or skin that does not follow the bones. `[ ] pass  [ ] fail`

4. **PBS and Unlit materials.** In that world, find one PBS surface and one Unlit surface. Record the objects. Pass: the PBS surface responds to the scene lights and the Unlit surface stays a flat color. Fail: either surface is missing, black, or shows the other response. If the world only has one family, mark fail and name the family that was absent. `[ ] pass  [ ] fail`

5. **Lights and shadows.** Find a lit surface and a shadow from a caster in that world. Record both. Pass: the lit side is brighter and the shadow is visible on the receiver. Fail: a light is present and the surface stays flat, or an expected shadow is missing. `[ ] pass  [ ] fail`

6. **UI and text.** Open a Resonite UI surface that contains text. A nameplate with glyphs counts. Pass: glyphs are readable and the panel is drawn. Fail: missing glyphs, a blank panel, or overlapping garbage. `[ ] pass  [ ] fail`

7. **Mirrors, secondary cameras, render textures.** Desktop: if the world has a mirror or a camera that draws into a render texture, look at that surface. VR: the desktop window is a blit of one HMD eye, not a second world render. The renderer log line for that path is `GPU initialized (OpenXR Vulkan device + mirror surface)`. Pass: the mirror, render texture, or desktop eye view updates with the scene. Fail: black, frozen, or a desktop window that does not show the eye. Desktop with no in-world mirror and no render-texture camera: `[ ] n/a` and name the world. VR cannot use n/a for this item. `[ ] pass  [ ] fail  [ ] n/a`

8. **Video textures.** Run only on a binary built with `--features video-textures` and GStreamer installed. Play a local file or URI the world already uses. Pass: decoded frames appear in the texture. Fail: the texture stays black, or the renderer log shows a `CpuCopyVideoSink` warning for that playback. Feature off: `[ ] n/a` with the reason `video-textures feature off (placeholder stays black)`. Do not mark pass on the default build. `[ ] pass  [ ] fail  [ ] n/a`

9. **VR or desktop, the mode you launched.** Record the dialog line (`Desktop/VR dialog returned: Desktop.` or `VR.`) or the flag (`-Screen`, `-Device SteamVR`). Desktop pass: a window presents and the log contains `GPU initialized (desktop)`. VR pass: the headset presents and the log contains `GPU initialized (OpenXR Vulkan device + mirror surface)`. The mode you did not launch is n/a with the reason `not launched`. One mode is a complete item. A later pass should cover the other mode. `[ ] pass  [ ] fail  [ ] n/a`

10. **World switch and reconnect.** From the session, open a second world, wait until it draws, then return to the first. Pass: both worlds draw and the renderer process stays up. The renderer log does not gain `IPC connect failed` or `Timed out waiting for RendererInitData` after the first handshake. Fail: a black world, a hang, or the renderer exits. `[ ] pass  [ ] fail`

11. **Clean shutdown, no crash, no validation errors.** GPU validation layers must be on for this item (see Preconditions). Quit by closing the window or exiting the host. Pass: the renderer log contains `Graceful renderer shutdown completed`, and this session's renderer log has no panic, no `Fatal IPC error`, no `wgpu validation error`, no `render graph validation`, and no `surface validation error`. Vulkan validation text is forwarded into the same file. Fail if any of those lines appear, or if GPU validation layers were left off (write `GPU validation layers were off`). `[ ] pass  [ ] fail`

12. **Rough frame time.** F7 shows the ImGui UI. Renderer config, Debug tab, enable `Frame timing HUD`. Stand still in the loaded world and read the Frame timing window (`FPS` and `Frame`). Record both. There is no millisecond gate in this checklist, and the numbers are not comparable to lavapipe `cpu_frame_ms`. Pass: the window showed numbers and the session stayed interactive. Fail: the window was absent, or the session was not usable while you read it. `[ ] pass  [ ] fail`

## Required evidence

Attach all of the following to the result record. A missing piece makes the pass unverified.

- Renderide full SHA (`git rev-parse HEAD`) and the log `commit_sha8`.
- Resonite version, or the Steam `buildid` from `appmanifest_2519830.acf`.
- GPU, driver, and `driver_info` from the `GPU: adapter=` line, plus OS from `Renderer host system:`.
- Log excerpt or the log file path (`Logging to ...`). Include the handshake lines and the shutdown lines.
- At least one screenshot for each visual area: local home world, avatar, PBS surface, Unlit surface, lights and shadows, UI text, mirror or render texture (or the VR desktop eye), video texture when item 8 is not n/a, the launched mode, the second world, and the Frame timing window.
- Each failure filed as an issue on this fork, with the issue link in the record.

## Result record

Copy this block. Fill every field. Delete no heading.

```
### YYYY-MM-DD

- Status: pass | fail | unverified
- Operator:
- Renderide SHA:
- commit_sha8 (log):
- Resonite version or buildid:
- Resonite dir:
- OS:
- GPU / driver / driver_info / backend:
- Binary: source | release zip
- video-textures: on | off
- Mod loaded: yes | no
- Mode: desktop | vr
- GPU validation layers: on | off
- Log file:

| # | Item | Result | Notes |
| --- | --- | --- | --- |
| 1 | Launch and handshake | pass/fail | |
| 2 | Local home world | pass/fail | world name |
| 3 | Avatar and skinned meshes | pass/fail | |
| 4 | PBS and Unlit | pass/fail | objects |
| 5 | Lights and shadows | pass/fail | |
| 6 | UI and text | pass/fail | |
| 7 | Mirrors / secondary cameras / render textures | pass/fail/n/a | |
| 8 | Video textures | pass/fail/n/a | |
| 9 | VR or desktop | pass/fail/n/a | |
| 10 | World switch and reconnect | pass/fail | |
| 11 | Clean shutdown and validation | pass/fail | |
| 12 | Rough frame time | pass/fail | FPS, Frame ms |

Screenshots:

- home:
- avatar:
- pbs:
- unlit:
- lights:
- ui:
- mirror or render texture:
- video:
- mode:
- second world:
- frame timing:

Failures (issue links):

Log excerpt (handshake and shutdown):
```

## Pass log

### 2026-10-31

Unverified informal pass. The project owner (CTO, Styxx) reported, via Kairo, that a manual interop session on master around `e654ceb2d2cd4223681e554ebde10b9ac8c80571` (`e654ceb2`) "works well".

No checklist item was marked. No log excerpt was kept. No screenshot was kept. This entry is not evidence that any item above passed.

Future passes use the checklist and the result record. Do not add another informal line.
