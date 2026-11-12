# Remove ComfyUI's data folder only when the user explicitly asks

User-data safety fix. Since task 013, the managed engine keeps ComfyUI's writable folders (`user`, `input`, `output`, `temp`) in `<engineDir>/data`. `ai.setup.remove` now deletes that folder whenever the request removes the engine and every manifest model (`let everything = all_ids.iter().all(|id| models.contains(id));` in `crates/engine/src/ai_setup_cmds.rs`, `fn remove`, about lines 786–830). That silently deletes the user's ComfyUI workflows and output as a side effect of removing models. If the manifest had no models, `all()` would also be true. Data removal must be its own explicit choice.

Scope:

- `crates/engine/src/ai_setup_cmds.rs`, `fn remove`: add a boolean `data` parameter, default `false`, parsed with `flag_param` like `engine`. Call `runtime::remove_engines(engine_dir, engine && data)`. Delete the `everything` inference. `data: true` without `engine: true` is an actionable `Err` (never a panic). Update the `ai.setup.remove` entry in `specs()` (search for `label:` near the other `ai.setup` specs) so `params` documents `"data":bool=false` and what it deletes.
- `crates/aisetup/src/runtime.rs`, `pub fn remove_engines(engine_dir: &Path, everything: bool)` (about line 362): rename the parameter to `remove_data` and keep its behaviour; update its doc comment.
- `crates/ui-egui/src/ai_setup_ui.rs`: the remove-everything call `app.session.execute("ai.setup.remove", json!({"engine": true, "models": models}))` (about line 863). Add a separate, unticked-by-default checkbox in that confirmation ("Also delete ComfyUI's saved workflows and output"), and pass `"data": <checkbox>`. Do not change the single-model remove at about line 877.
- Windows behaviour unchanged apart from the new parameter; Windows engines have no `data` folder.

Tests (each must fail without the fix):

- Engine: remove with `engine: true, models: true` and no `data` keeps `<engineDir>/data/user/...` files and reports the freed bytes without them.
- Engine: `engine: true, models: true, data: true` removes `<engineDir>/data`.
- Engine: `data: true` without `engine: true` is an `Err` and removes nothing.
- `remove_engines(dir, false)` keeps `data`; `remove_engines(dir, true)` removes it (adapt the existing test in `crates/aisetup/src/runtime.rs` near line 661).

Name engine tests with `ai_setup` in their path or name. Edit only the files above and their existing test modules (`crates/engine/src/ai_setup_cmds/tests.rs`, `crates/ui-egui/src/ai_setup_ui/tests.rs`). Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
