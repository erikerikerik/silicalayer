# Suggest only MPS-qualified models on Apple Silicon

Correctness fix. Since task 012, a Mac reports its unified memory as `vramGb`, so a 24 GB Mac gets the `recommended` preset (`qwen-image-2-1`, `z-image-turbo`, `flux-3-klein` in `crates/aisetup/manifest.json`). Unified memory is shared with macOS and the editor, so it is not equivalent to a card's VRAM, and only Z-Image Turbo has been qualified on MPS (`docs/MPS_MODEL_SPIKE.md`). A Mac must be offered the qualified model first.

Scope:

- `crates/aisetup/manifest.json`: add a preset `"apple-silicon": ["z-image-turbo"]`. Do not change any other preset or model entry.
- `crates/aisetup/src/manifest.rs`: `pub fn suggested_preset(&self, vram_gb: Option<u64>)` (about line 316) and `pub fn offered` (about line 323). Add a platform-aware form, e.g. `suggested_preset_for(platform: Option<RuntimePlatform>, vram_gb: Option<u64>)` and `offered_for(...)`: on `Some(RuntimePlatform::MacosArm64)`, return `"apple-silicon"` when that preset is non-empty, whatever the memory; otherwise exactly today's rule. Keep the old functions as thin wrappers with `None` so Windows callers are unchanged. `RuntimePlatform` is in `crates/aisetup/src/platform.rs`.
- `crates/engine/src/ai_setup_cmds.rs`, `fn manifest_value` (about lines 60–95, the `.offered(vram_gb)` and `"suggested": m.suggested_preset(vram_gb)` calls): pass `photocraft_aisetup::RuntimePlatform::host()`.
- `crates/ui-egui/src/ai_setup_ui.rs`, `fn suggested_preset` (about line 367): if it computes the preset itself rather than reading the engine's `suggested`, apply the same platform rule.
- The other models stay listed and installable on a Mac; only the suggestion and first-listed order change.

Tests (each must fail without the fix), in `crates/aisetup/src/manifest.rs`'s test module (see the existing preset test near line 683):

- MacosArm64 with 8, 16, 24 and 64 GB, and with unknown memory: suggested preset `apple-silicon`; `offered_for` lists `z-image-turbo` first.
- `None` and WindowsPortable: exactly today's results for the same memory figures (`light` below 16, otherwise `recommended`).
- A manifest without an `apple-silicon` preset on a Mac falls back to today's rule.

## Macs below the 24 GiB floor: install allowed, cloud recommended (Erik's decision)

A Mac with less than `MIN_QUALIFIED_UNIFIED_MEMORY_MB` (24 GiB, `crates/aisetup/src/probe.rs`) or with unknown memory can still install the local engine. The wizard must recommend cloud models for speed instead:

- `crates/aisetup/src/probe.rs`, the `gpu` check row's `Warn` text for an unqualified Mac (about line 417, "Local models may not fit…"): say plainly that local generation will be slow or may not fit on this Mac, and that cloud models are recommended for speed; installing locally is still possible. Keep the level `Warn` (not `Stop`) and keep `can_install()` true.
- `crates/ui-egui/src/ai_setup_ui.rs`, the PC page's left column (about line 2602, the `"Cloud Models Only"` secondary button that calls `crate::ai_gen_ui::open_api_keys(app)`): when the probe's runtime target is `macosArm64` and not qualified, show that button as the recommended choice (primary styling or a "Recommended for this Mac" label) and add a one-line note above the local install path. Use the probe JSON the page already has (`runtimeTarget.qualified`); do not add a new command.
- Recommendation only. Never choose a cloud provider, enter a key, or route generation to the cloud automatically: the user must click and add their own API key. No cloud provider becomes a fallback for a failed or slow local run.
- Qualified Macs (24 GiB and above) and Windows: unchanged.

Extra tests (each must fail without the change):

- Probe: an 8 GB and an unknown-memory Mac give a `Warn` row whose detail mentions cloud models being recommended and still allows install; a 24 GB Mac's row is unchanged.
- UI (`crates/ui-egui/src/ai_setup_ui/tests.rs`): with an unqualified macOS probe the PC page marks Cloud Models Only as recommended; with a qualified macOS probe or a Windows probe it does not. Nothing in either case opens the API-key window or changes provider settings by itself.

Edit only these files (plus `crates/aisetup/src/probe.rs` and `crates/ui-egui/src/ai_setup_ui/tests.rs`) and their existing test modules. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.

## Remaining gate failures after the first run (fix only these)

The first run's code is in the working tree. The engine and `aisetup` parts pass. Three `ui-egui` setup tests fail on the Mac. Do not touch `crates/ui-egui/src/tools/` or any file not listed in this task: an unrelated font-menu change was removed from this task's diff.

1. `a_small_card_starts_on_the_suggested_preset` and `opening_fills_in_the_folders_and_the_recommended_models` get preset `"custom"` instead of `"apple-silicon"`. Cause: `crates/ui-egui/src/ai_setup_ui.rs` has a fixed `const PRESETS: [(&str, &str); 4]` (about line 2045). It is used both to draw the preset pills in `fn page_models` and to name the current selection (`PRESETS.into_iter().map(|(key, _)| key).find(|p| same(p)).unwrap_or("custom")`, about line 1305). Fix: build the preset list at runtime, as `PRESETS` plus `("apple-silicon", "Apple Silicon")` first when the manifest's `/presets/apple-silicon/models` is non-empty and `RuntimePlatform::host() == Some(RuntimePlatform::MacosArm64)`. Use that list in both places. Windows shows exactly the four presets it shows today.
2. `an_installed_setup_can_lose_a_model_or_everything_without_being_taken_for_a_new_one` (about line 406 of `crates/ui-egui/src/ai_setup_ui/tests.rs`) compares the `installed` flags by list position. On a Mac the list now starts with `z-image-turbo`, so the positions swap. Fix the test, not the order: read `installed` by model id (a map from `id` to `installed`) and assert `z-image-turbo` is `false` and the other model `true`.

Keep `rustfmt` clean: wrap long lines the way the surrounding code does.
