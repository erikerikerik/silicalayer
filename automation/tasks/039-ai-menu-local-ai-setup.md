# AI menu also lists Local AI Setup…, under Style Presets…

Erik's AI menu (task 036) lists Generate Image…, Inpaint Selection…, Restyle Layer…, Style Presets…, Local AI Setup… and Generative AI…. 036 left Local AI Setup… out, for a good reason: `ai_setup_ui::insert_menu_items` adds that item (`ai.setup.open`, with its dynamic label "Set Up Local AI…" / "Local AI Setup…") to the File menu at run time, and skips adding it when the id is already in the tree, so a static catalog entry would have taken it out of File. Fix it where the item is made: add it to both menus.

Scope:

- `crates/ui-egui/src/ai_setup_ui.rs`, `insert_menu_items` (about line 495):
  - Keep everything it does today for File (the early return when `OPEN` is already in the list, the File insertion above `file.fileInfo`, the separator, the label and `available(app)`).
  - Then, also insert one more item with the same id `OPEN`, the same label and the same enabled state, with `path: vec!["AI".to_string()]`, directly after the item whose id is `ai.menu.styles` (so it sits between Style Presets… and Generative AI…). If no `ai.menu.styles` item is in the list, insert nothing more. No separator for this one.
  - Compute the AI position after the File insertion (indices shift). Update the doc comment to say the item appears in File and in AI.
  - The same id in two menus is fine here: `edit.preferences.generativeAi` already appears in two menus, and a click dispatches by id.

- `crates/ui-egui/src/ai_setup_ui/tests.rs`: add tests:
  - after `insert_menu_items` on the real menu list (as the existing menu tests build it), `ai.setup.open` appears once under path `["File"]` and once under path `["AI"]`, and the AI one comes right after `ai.menu.styles` and right before `edit.preferences.generativeAi`;
  - both carry the same label (try both states if the existing tests already switch `set_up`; otherwise one state is enough);
  - with no `ai.menu.styles` in the list, only the File entry is added.
  Keep every existing test passing; update any that counts `ai.setup.open` occurrences and assumes one.

Edit only the files named above. Do not add files. Do not open image or binary files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
