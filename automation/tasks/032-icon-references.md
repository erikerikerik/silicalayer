# Point the title-bar mark and the last references at the new SilicaLayer icon

The new app icon landed in `assets/app-icon/` (see its README). Four things still use LayerShop's old icon files, `assets/app-icon/photocraft.svg` and `assets/app-icon/photocraft-small.svg`, which this task removes.

Scope:

- `crates/ui-egui/src/panels/brand.rs` (about line 62): `ICON_PNG` includes `assets/app-icon/silicalayer-mark-128.png` (the simplified crystal on a full-bleed tile, 128 px) instead of `hicolor/128x128/apps/ai.storyteller.photocraft.png`. Keep the comment above it accurate (128 px, mipmapped). Update the module doc comment (about lines 8–10) so it names `silicalayer-mark-128.png`. Any test in that file that decodes the mark must still pass.
- `crates/io/src/svg/tests.rs` (about lines 348–354): the SVG import check that opens the app icon uses `assets/app-icon/silicalayer-small-tile.svg` instead of `photocraft.svg` (both the `include_bytes!` path and the name passed to `import_svg`). It has no `clipPath`, so the existing `if` branch is skipped; keep the "no clip-path warning" assertion. Update the two comments there to say it is SilicaLayer's icon.
- `docs/releasing.md`, the "Icons" section (about lines 421–427): say the masters are the four files in `assets/app-icon/` (`silicalayer-1024.png`, `silicalayer-mac-1024.png`, `silicalayer-small.svg`, `silicalayer-small-tile.svg`), and that `packaging/icons.sh` needs `resvg` and `sips` (macOS) or ImageMagick, plus `iconutil` on macOS. Remove the kitsune sentence.
- Delete `assets/app-icon/photocraft.svg` and `assets/app-icon/photocraft-small.svg`.
- `ATTRIBUTION.md` (about lines 78–79): drop the row for those two files, and change the first row's path cell to `` `assets/app-icon/` (all files) ``.

Checks: `git grep -n 'photocraft\.svg\|photocraft-small\.svg'` finds only `hicolor/scalable/apps/ai.storyteller.photocraft.svg` paths (that file stays).

Edit only the files named above. Do not add files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes (it builds the app and runs the ui-egui and io tests).
