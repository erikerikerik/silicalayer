# SilicaLayer app icon

**A glowing crystal on three translucent layers**: silica (the crystal) and layers, the two halves
of the name, in the spirit of the illustrative emblems of early Creative Suite icons. The crystal
glows cyan to magenta; the layers below it carry the same gradient.

This fork ships as SilicaLayer, so this folder holds its icon in place of LayerShop's three
sheets (and upstream PhotoCraft's kitsune before that). The output file names are unchanged
(`photocraft.*`, `ai.storyteller.photocraft.*`): they are machine names, and keeping them means
the build, the tests and the packaging scripts need no edits.

## Masters

| File | What it is | Used for |
|---|---|---|
| `silicalayer-1024.png` | The crystal art, free-form on transparency | Windows and Linux, 40 px and up |
| `silicalayer-mac-1024.png` | The same art on a dark indigo tile (`#2a2075` → `#0c0926`), 824 px body with a 185 px corner radius, on Apple's 1024 px grid | macOS, 128 px and up |
| `silicalayer-small.svg` | A simplified drawing of the same idea (two crystals, three plates) on a 512-unit canvas | 32 px and below on Windows and Linux, and the scalable Linux icon |
| `silicalayer-small-tile.svg` | The simplified drawing on a full-bleed tile (`rx=112`) | macOS up to 64 px (padded onto the 824/1024 grid), and the title-bar mark |

The detailed art turns to mush below about 40 px, so small sizes use the simplified drawing: the
crystal keeps two flat facets and a white tip, and dark gaps between the plates keep the three
layers readable at 32 px.

macOS 26 puts any icon that doesn't fill Apple's rounded-square body into a grey tile of its own,
so the Mac icon carries its own tile. Windows and Linux show the art free-form.

## Files

- `photocraft-1024.png`: a copy of `silicalayer-mac-1024.png` (the window and Dock icon of
  unbundled macOS runs, `apps/photocraft/src/app_icon.rs`).
- `photocraft.icns`: macOS bundle icon (`CFBundleIconFile`), 16–1024 px.
- `photocraft.ico`: Windows icon, 16–256 px, embedded in the `.exe` by `apps/photocraft/build.rs`.
- `hicolor/<size>/apps/ai.storyteller.photocraft.png` (16–512) and `hicolor/scalable/...svg`:
  Linux icon theme. The 256 px one is the window icon on Windows and Linux.
- `silicalayer-mark-128.png`: the mark at the left of the title bar
  (`crates/ui-egui/src/panels/brand.rs`), where Photoshop shows its "Ps" tile.

On Windows the taskbar shows the icon of the Start Menu shortcut that launches the exe, so the
MSI's `<Icon Id>` keeps the `.exe` extension (ICE50); `app_icon.rs` documents the details and tests
the `.ico` sizes and the WiX icon references.

## Regenerate

`packaging/icons.sh` writes every output from the four masters. It needs `resvg`, and `sips`
(macOS) or ImageMagick; on macOS `iconutil` writes the `.icns`.

License: see `LICENSE.txt`.
