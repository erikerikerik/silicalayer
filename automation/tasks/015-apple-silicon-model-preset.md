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

Edit only these files and their existing test modules. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
