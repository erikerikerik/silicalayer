# One-click Install for missing models; hide what the setup can't install

End-user fix from hands-on testing. In AI model settings (`crates/ui-egui/src/ai_gen_ui.rs`), a local model whose files are missing shows "Not installed" (about line 3712), sits in the red "Not Installed" group (`Group::Missing`, about lines 2726–2750) and, when selected, a box telling the user to download files into ComfyUI's models folder and press Scan Server (about line 2511). An ordinary user wants to click Install. The Local AI setup can already download every model in its manifest (`crates/aisetup/manifest.json`); setup models and settings models share a family (`photocraft_aigen::Family`, the manifest's `family` field; see `crates/aisetup/src/configure.rs`, `fn installed` and `fn managed`).

Scope, when the app manages its own engine (the setup record's route is `managed`):

- A missing local model whose family matches a manifest model gets an **Install** primary button in its row and in the selected-model panel, replacing the "Download it into ComfyUI's models folder…" box. Install opens the Local AI setup wizard (`crate::ai_setup_ui::open`) with that manifest model added to the wizard's selected models (`app.ui.ai_setup.models`), on its Models page. It downloads nothing by itself; the wizard shows size and licence as today.
- A missing local model that no manifest model can install is not listed in the main groups. Put such models in a collapsed group at the bottom titled "For your own ComfyUI", with one line explaining that these need a ComfyUI you manage yourself. The manual-download box appears only inside that group.
- "Remove Not Installed…" (about line 3806) keeps working for both kinds.
- When the user runs their own ComfyUI (route `existing`) or no setup exists, today's behaviour and wording stay exactly as they are.

Tests (each must fail without the change), in this file's existing test module (`crates/ui-egui/src/ai_gen_ui/tests.rs`):

- Managed setup, `z-image-turbo` installed: a settings model of the Qwen Image 2.1 family that is missing shows Install; clicking it opens the setup wizard with `qwen-image-2-1` among its selected models.
- Managed setup: a missing FLUX.1 Fill model is listed only under "For your own ComfyUI", collapsed, and the main view shows no manual-download box.
- Existing-ComfyUI setup: both show today's "Not installed" row and box.

Edit only `crates/ui-egui/src/ai_gen_ui.rs` and `crates/ui-egui/src/ai_gen_ui/tests.rs` (and `crates/ui-egui/src/ai_setup_ui.rs` only if a small public helper is needed to open the wizard with a model preselected). Do not add files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
