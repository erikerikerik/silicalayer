# Restyle: style presets shown under the prompt, not hidden behind the Style button

Depends on task 034 (the Restyle tab). Erik's hands-on report: on Restyle the look is the whole point, but the 24 style presets only appear after clicking the small Style button in the bottom row (`styles::button`, `crates/ui-egui/src/ai_gen_ui/styles.rs` about line 118, called from `ai_gen_ui.rs` about line 2100). On the Restyle tab they should be in view as soon as the tab opens.

Scope (in `crates/ui-egui/src/ai_gen_ui/styles.rs`, `crates/ui-egui/src/ai_gen_ui.rs` and `crates/ui-egui/src/ai_gen_ui/tests.rs` only):

- Add `pub(super) fn strip(app: &mut PhotocraftApp, ui: &mut egui::Ui, enabled: bool)` in `styles.rs`: a "Style" label (dim, like the panel's other field labels), then one row of square tiles that fills the available width and never scrolls sideways:
  - Tile size: as many tiles as fit at 64–84 px with the existing `GAP`, at least 3. The tiles are, in order: No preset, the chosen preset (when one is chosen), then the presets in `THUMBS` order skipping the chosen one, and last a "More…" tile. Show as many as fit, keeping No preset, the chosen preset and More… always.
  - Reuse `tile` (or a smaller variant of it) so a tile looks and behaves like the picker's: thumbnail, name under it (truncated to the tile's width), the chosen one framed in the accent with the check, the larger preview on hover. Clicking a tile sets `app.ui.ai_gen.style` exactly as the picker does.
  - The More… tile (a plain tile with a "grid" or "more-horizontal" icon from the existing icon set, and the word "More…") opens the existing picker popup (`picker`), anchored to that tile, with the same close behaviour as the Style button's popup.
  - Disabled (`enabled == false`): drawn dimmed, not clickable.
- In `ai_gen_ui.rs`, on the Restyle tab only, draw `styles::strip` directly under the prompt box, before the fields that follow it. On Restyle, the bottom row's `styles::button` is not drawn (the strip replaces it). Create and Inpaint keep the Style button as now.
- Tests: the strip draws at a narrow (280 px) and a wide (480 px) panel width without panicking and with at least 3 tiles; clicking the More… tile opens the picker; choosing a tile sets the style. Follow the file's existing UI test pattern (a fresh egui context's first frame must clear `textures_delta`).

Edit only those three files. Do not add files. Do not open image or binary files (the thumbnails are already embedded by `THUMBS`). Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes, including the ai_gen_ui tests.
