# AI Gen: three tabs, Create | Inpaint | Restyle, each with a visible one-line description

Erik's hands-on report: "Edit" doesn't say what it does, and Restyle is big enough to be a tab of its own. Today the panel (`crates/ui-egui/src/ai_gen_ui.rs`) has two tabs, `Mode::Create` and `Mode::Edit` (about lines 43–73), and Edit has a second row, `EditTask::Change` / `EditTask::Restyle` (about lines 75–105), drawn by `task_row` (about line 667). The tabs are drawn by the function ending about line 664, which splits the width evenly over `Mode::ALL`.

Scope (in `crates/ui-egui/src/ai_gen_ui.rs` and its tests `crates/ui-egui/src/ai_gen_ui/tests.rs` only):

- `Mode` becomes three variants: `Create`, `Inpaint`, `Restyle`. `Mode::ALL` lists the three in that order; labels "Create", "Inpaint", "Restyle".
  - Serde: keep `rename_all = "camelCase"` and `#[default] Create` with its `txt2img` alias. `Inpaint` takes the aliases `edit`, `inpaint` and `imgEdit`; `Restyle` takes `img2img`. A saved state whose mode is `edit` loads as Inpaint.
  - Tips: Inpaint uses today's `EditTask::Change` tip, Restyle today's `EditTask::Restyle` tip, Create keeps its tip.
- Remove `EditTask` and `task_row`. Every place that tests `Mode::Edit` together with `EditTask::Change` now tests `Mode::Inpaint`; with `EditTask::Restyle`, `Mode::Restyle`; `Mode::Edit` alone (both) becomes `matches!(mode, Mode::Inpaint | Mode::Restyle)`. Behaviour for each case stays exactly as it is now (models offered, strength and context fields, mask tools, source layer, result layer).
  - The `AiGenUi` field that held the `EditTask` goes. If `AiGenUi` (or anything containing it) uses `#[serde(deny_unknown_fields)]`, keep loading old states: an old state with that field must still deserialize (it may be ignored).
- Under the tab row, and under Create's Prompt/Regions row when Create is active, draw one line of description for the active tab, always visible, wrapping to the panel width, in `t.text_dim` at the size of the panel's other small text:
  - Create: "A new image from your prompt, into the selection or the whole canvas."
  - Inpaint: "Select an area, then describe what goes there: only the selection changes. With no selection, the whole picture changes as the prompt says."
  - Restyle: "Redraws the whole picture in a new style and keeps its composition. Strength sets how far it may drift."
  - Pass each through `tr()` as the panel's other strings are.
- Update every comment and doc comment that names Edit, Change or `EditTask` to the new tabs.
- Tests: update `tests.rs` to the new variants (about 33 places). Add one test that `serde_json::from_str::<Mode>("\"edit\"")` is `Mode::Inpaint` and `"\"img2img\""` is `Mode::Restyle`, and one that a 3-tab panel draws without panicking (follow the file's existing UI test pattern; a fresh egui context's first frame must clear `textures_delta`).

Edit only those two files. Do not add files. Do not open image or binary files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes, including the ai_gen_ui tests.
