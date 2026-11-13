# Show the product as SilicaLayr, credit its lineage, and give the Mac one data folder

Phase 5 branding, step 1, following the pattern LayerShop already uses. `crates/ui-egui/src/panels/brand.rs` holds the product name (`pub const NAME: &str = "LayerShop";`), the About credit (`pub const BASED_ON: &str = "Based on PhotoCraft by the ArtCraft team.";`) and `fn display`, which turns "PhotoCraft" in UI strings into `NAME` on the way to the screen, so no catalog or upstream string is edited. Binaries, crate names, ProgIds, file formats and bundle identifiers keep their PhotoCraft machine names; this task changes none of them.

Scope:

- `crates/ui-egui/src/panels/brand.rs`: `NAME = "SilicaLayr"` (the brand is spelled without the second "e"; repository, crate and binary names keep `silicalayer`/`photocraft`). `BASED_ON` = "Built on the PhotoCraft engine by the ArtCraft team, with workflows from LayerShop by Slavomir Durej. The Apple Silicon port is SilicaLayr's own." Update the module doc comment ("This fork ships as LayerShop") to say SilicaLayr, a fork of LayerShop, itself a fork of PhotoCraft. `display` must also turn "LayerShop" in UI strings into `NAME` (strings this fork's code added say LayerShop), without touching `BASED_ON`.
- `apps/photocraft/src/app_dirs.rs`: today `pub const DIR_NAME: &str = photocraft_ui_egui::brand::NAME;` (about line 25) names the settings folder and, through `fn local_ai_dirs`, the Local AI engine and models folders. Keep Windows, Linux and the other platforms on the folder name "LayerShop" exactly as today (their installers and existing users depend on it): give the folder name its own constant, "SilicaLayr" on macOS and "LayerShop" elsewhere. Portable and `PHOTOCRAFT_CONFIG_DIR` modes are unchanged.
- One-time macOS migration, before anything reads settings: if `~/Library/Application Support/SilicaLayr` does not exist and `~/Library/Application Support/LayerShop` does, rename the folder (one `rename` on the same volume; never copy-and-delete). Then, in the moved `ai-setup.json`, rewrite every stored path (`engineDir`, `modelsDir`, `extraModelsDirs`, and any other absolute path the setup record holds) that starts with the old folder to start with the new one. If both folders exist, use the new one and leave the old one untouched. If the rename fails, keep using the old folder for this run and log why; never lose or half-move data.
- Paths the engine wrote inside its own folder (the extra-model-paths config) are rewritten the next time the app starts the engine or a setup runs; if that is not already true, make the launch path regenerate that config from the current layout.
- `NOTICE`: add one line saying SilicaLayr (repository `silicalayer`) is a fork of LayerShop, itself a fork of PhotoCraft, and that their licences and notices are kept.

Tests (each must fail without the change):

- `brand::display("Restart PhotoCraft")` and `brand::display("LayerShop setup")` give the SilicaLayr forms; `BASED_ON` is unchanged by `display`.
- `app_dirs`: with a temp home, macOS rules give `…/Application Support/SilicaLayr` and Local AI folders under it; the Windows and Linux rules still give the LayerShop folders.
- Migration (pure function taking the two paths, tested with temp folders): old only → renamed, record paths rewritten, nothing else changed; new only → untouched; both → new used, old untouched; neither → nothing created; a record path outside the old folder (a models folder the user chose) is not rewritten.

Edit only `crates/ui-egui/src/panels/brand.rs`, `apps/photocraft/src/app_dirs.rs`, `apps/photocraft/src/main.rs` (only to call the migration at start-up), `NOTICE`, and their existing test modules. Do not change bundle identifiers, binary names, `packaging/` or the app icon. Do not add files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
