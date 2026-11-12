# Offer only the runtime builds for the host platform

P1 bug fix. On macOS the setup can still choose, plan, download and unpack a Windows NVIDIA build: the install/plan command defaults `platform` to the saved record (default `windowsPortable`) and `variant` to `nvidia` when they are absent (`crates/engine/src/ai_setup_cmds.rs`, around the `text_param(p, "platform", cmd)` match), and the UI falls back to `"nvidia"` (`crates/ui-egui/src/ai_setup_ui.rs`, `fn params`). A caller can also request `macosArm64` on Windows.

Scope:

- In `crates/aisetup/src/platform.rs` add `RuntimePlatform::host() -> Option<RuntimePlatform>`: `WindowsPortable` under `cfg!(windows)`, `MacosArm64` under `cfg!(all(target_os = "macos", target_arch = "aarch64"))`, otherwise `None`. Put the decision logic in a pure helper that takes the host platform as a parameter so every branch is unit-testable on any machine.
- Add `Manifest::assets_for(platform)` returning only the assets whose key `platform.accepts_variant`. Use it wherever setup lists, defaults or validates managed builds.
- In the engine command: default `platform` to the host platform and `variant` to that platform's default (`macos_arm64` on macOS; the probe's choice, else `nvidia`, on Windows). Reject an explicit `platform` other than the host's, and a variant the host platform does not accept, with an actionable `Err` (never a panic). A saved record naming another platform is not reused for its platform or variant.
- Remove the UI's hard-coded `"nvidia"` fallback; leave the variant to the engine's host default when no probe exists.
- On a host with no managed platform (Linux, Intel Mac) the managed route stays unavailable; "Use my ComfyUI" is unchanged.
- Do not delete Windows code, Windows manifest entries or Windows tests. Windows behavior stays identical.

Tests (each must fail without the fix), using the pure helper with an explicit host platform:

- macOS host, no params: platform `macosArm64`, variant `macos_arm64`.
- macOS host, `platform: windowsPortable` or `variant: nvidia`: `Err`, nothing planned.
- Windows host, `platform: macosArm64`: `Err`.
- macOS host with a saved `windowsPortable`/`nvidia` record: not reused.
- `assets_for(MacosArm64)` is exactly `macos_arm64`; `assets_for(WindowsPortable)` excludes it.

Exit evidence: the full supervisor gate passes, including the engine and UI setup tests.

## Exact locations (verified; do not search for other files)

Edit only these files. Line numbers are approximate; read each file before editing it, and copy the text you replace exactly from the file.

- `crates/aisetup/src/platform.rs`: `pub enum RuntimePlatform` (about line 14) and `fn accepts_variant` (about line 35). Add `host()` and the pure helper here, with their unit tests in this file's `#[cfg(test)]` module.
- `crates/aisetup/src/manifest.rs`: `pub struct Manifest` (about line 149) and `impl Manifest` (about line 195). Add `assets_for` in that `impl`, with tests in this file's test module.
- `crates/engine/src/ai_setup_cmds.rs`: the `text_param(p, "variant", cmd)` and `text_param(p, "platform", cmd)` handling (about lines 382–390). Name any new engine tests with `ai_setup` in their path or name so the gate's `ai_setup` filter runs them.
- `crates/ui-egui/src/ai_setup_ui.rs`: `fn params` (about line 735); the `"nvidia"` fallback is about line 755.

There is no `crates/engine/src/ai.rs`. Do not read `target/`, build output, or anything outside this repository.
