# Mac-aware model fit and wording in the setup wizard

Polish with a correctness edge. On an Apple Silicon Mac the setup wizard still speaks of "this PC" and "your card", and it marks a model `tested` when it ran on an NVIDIA card no larger than the Mac's unified memory. That is misleading: only the models in the manifest's `apple-silicon` preset (today `z-image-turbo`) have been run on MPS (`docs/MPS_MODEL_SPIKE.md`).

Scope:

- `crates/engine/src/ai_setup_cmds.rs`, `fn fit` (about line 58) and its caller `fn manifest_value` (about line 71, which already has `let host = photocraft_aisetup::RuntimePlatform::host();`): give `fit` the host platform and the manifest's `apple-silicon` preset. On `Some(RuntimePlatform::MacosArm64)`: `tooLarge` when the model's `min_vram_gb` exceeds the unified memory (unchanged rule), `tested` only when the model is in the `apple-silicon` preset and the memory is at least `MIN_QUALIFIED_UNIFIED_MEMORY_MB` (`crates/aisetup/src/probe.rs`), otherwise `untested`. Every other host: exactly today's rule. Update the `fit:` and `vramGb` wording in that command's `params` text in `specs()` only if it would now be wrong.
- `crates/ui-egui/src/ai_setup_ui.rs`, user-facing text only, chosen at run time from `RuntimePlatform::host()` (import as the file already does): on an Apple Silicon Mac say "Mac" instead of "PC" in the strings at about lines 649, 1602, 1708, 2277, 2344 and 2345, and in the model rows (about lines 2144–2151) say "unified memory" instead of "card" ("suggested for your 24 GB Mac", "too large for this Mac's 24 GB of memory", "not tested on Macs"). The message at about line 1129 ("No graphics card the engine can use was found…") is not shown on an Apple Silicon Mac; leave it as it is. Windows text stays word for word as today.
- Keep each string passed through `tr(...)` as it is today; add the Mac variants as separate `tr(...)` strings.

Tests (each must fail without the change):

- Engine (name containing `ai_setup`): with a MacosArm64 host and 24 GB, `z-image-turbo` is `tested`, `qwen-image-2-1` is `untested` (even though it was tested on a 24 GB card), and a model with `min_vram_gb` above 24 is `tooLarge`; with 16 GB, `z-image-turbo` is `untested`. With a Windows host the results are exactly today's. If `fit` needs the host passed in to be testable on any machine, pass it in.
- UI (`crates/ui-egui/src/ai_setup_ui/tests.rs`): on a macOS host the checking screen title and the model row hints use the Mac wording; existing Windows-wording assertions keep passing on non-macOS hosts (use the same `#[cfg(all(target_os = "macos", target_arch = "aarch64"))]` pattern the file already uses).

Edit only `crates/engine/src/ai_setup_cmds.rs`, `crates/engine/src/ai_setup_cmds/tests.rs`, `crates/ui-egui/src/ai_setup_ui.rs` and `crates/ui-egui/src/ai_setup_ui/tests.rs`. Do not add files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
