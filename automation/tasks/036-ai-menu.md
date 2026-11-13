# A top-level AI menu: Generate, Inpaint, Restyle, styles, setup and preferences

Depends on tasks 034 (tabs `Mode::Create`, `Mode::Inpaint`, `Mode::Restyle`) and 035 (the Restyle style strip). Erik approved a top-level **AI** menu so the generative features are found from the menu bar, as Window › AI Gen alone hides them.

Menu, between Filter and View, mnemonic `A`:

| Label | Command id | Does |
|---|---|---|
| Generate Image… | `ai.menu.create` | Shows the AI Gen panel (as Window › AI Gen's `{"show": true}` does) on the Create tab |
| Inpaint Selection… | `ai.menu.inpaint` | Shows the panel on the Inpaint tab. Enabled only when the active document has a selection |
| Restyle Layer… | `ai.menu.restyle` | Shows the panel on the Restyle tab |
| --- | | |
| Style Presets… | `ai.menu.styles` | Shows the panel on the Restyle tab, where the style strip is |
| Local AI Setup… / Set Up Local AI… | `ai.setup.open` | The existing command, with its existing dynamic label |
| Generative AI… | `edit.preferences.generativeAi` | The existing Preferences page |

Scope:

- `crates/ui-egui/src/menus.rs`: `TOP_MENUS` gets `"AI"` between `"Filter"` and `"View"` (11 entries; fix every array type and test that assumes 10).
- `crates/ui-egui/src/menus/menu_keys.rs`: `TOP_PREFERRED` gets `("AI", 'A')` in the same place; its mnemonic test becomes `"FEILYSTAVWH"`.
- `crates/ui-egui/src/menu_catalog.rs`: the AI menu's entries, in the order above, placed after the Filter entries and before View's. No default shortcuts.
- `crates/ui-egui/src/ai_gen_ui.rs`: `handles` also takes the four `ai.menu.*` ids; `menu` runs them (show the panel, set `app.ui.ai_gen.mode`), returning `{"mode": "create"|"inpaint"|"restyle"}`. If the panel already has a way to focus the prompt box, request it; otherwise don't add one. `ai.menu.inpaint`'s enabled state follows the active document's selection, wherever menu enabled state is decided for UI commands (see `crate::menus::is_enabled`).
- `ai.setup.open` and `edit.preferences.generativeAi` keep working where they are now. If the menu code requires each command id to appear in one menu only, leave those two out of the AI menu and say so in a comment in `menu_catalog.rs`; do not invent alias ids.
- Tests (next to the existing menu tests): the AI menu is between Filter and View; its items are live; each `ai.menu.*` command shows the panel on the right tab; Inpaint Selection is disabled with no selection and enabled with one. Update the native-menu and menu-navigation tests that iterate `TOP_MENUS` only as far as the new entry requires.

Edit only the files named above, `crates/ui-egui/src/menus/items.rs` if enabled state lives there, and the existing test files next to them. Do not add files. Do not open image or binary files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes, including the menus and ai_gen_ui tests.
