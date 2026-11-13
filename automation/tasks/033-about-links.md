# About and Help point at SilicaLayer, not at LayerShop's site and repository

Erik's hands-on report: About › "SilicaLayer website" opens LayerShop's website and "GitHub" opens `https://github.com/SlavomirDurej/layershop`. SilicaLayer has no website of its own; its home is its GitHub repository, `https://github.com/erikerikerik/silicalayer`.

Scope:

- `crates/ui-egui/src/links.rs`:
  - `APP_PAGE` and `GITHUB` = `https://github.com/erikerikerik/silicalayer`; `ISSUES` = `https://github.com/erikerikerik/silicalayer/issues`.
  - Add `pub const LAYERSHOP_WEBSITE: &str = "https://layershop.aardvarkstudios.net";` (the predecessor's site, credited by name).
  - Leave `TUTORIALS` (LayerShop's guides still apply to its workflows), `DISCORD`, `ARTCRAFT_WEBSITE` and `COMMUNITY_BUTTON` as they are.
  - `link_row` (about line 63) shows `GitHub · LayerShop · ArtCraft`: `(tl!("GitHub"), GITHUB), (tl!("LayerShop"), LAYERSHOP_WEBSITE), (tl!("ArtCraft"), ARTCRAFT_WEBSITE)`. Update its doc comment.
  - Update the module doc comment (lines 1–7): the fork ships as SilicaLayer; its own links (source, issues) are SilicaLayer's repository; LayerShop and ArtCraft are credited by name.
  - The tests at the end of the file keep passing; update any that name a changed URL.
- `crates/ui-egui/src/dialogs.rs` (about line 835, the About tab's first line): `tl!("PhotoCraft — an open-source image editor for Apple Silicon Macs, with private on-device AI generation.")` (`display()` turns "PhotoCraft" into the brand name).
- `crates/ui-egui/src/lib.rs` (about lines 15–28, `UPDATE_APP`): `developer: "erikerikerik"` and `repo: "erikerikerik/silicalayer"`; update the two comments above to say SilicaLayer instead of LayerShop. Nothing else in that struct changes.

Do not touch `apps/photocraft/src/updates.rs` (its feed is documented as must-change for the Phase 6 updater).

Edit only the three files named above. Do not add files. Do not open image or binary files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
