# Icon licence and attribution say how the crystal art was made

Erik's report: the crystal art is his, made in rounds with an AI image generator. He drew the first version, ChatGPT polished it, he edited that result and art-directed corrections, and ChatGPT polished those edits. The Mac tile and the two small-size SVGs were then made for SilicaLayer with Claude. The current text says "Original artwork… supplied by Erik Kristianson… No third-party material", which hides the AI steps. The licence and the attribution table must state them plainly.

Scope:

- `assets/app-icon/LICENSE.txt`, lines 5–8: replace the paragraph that starts `Original artwork for SilicaLayer:` and ends `Licensed like the rest of the project, under either of` with exactly this text (keep the line width near 98 characters like the rest of the file, and keep lines 1–3 and the licence list from line 10 on unchanged):

  ```
  The SilicaLayer app icon: a glowing crystal on three translucent layers. The crystal art is by
  Erik Kristianson. He drew the first version; ChatGPT (an AI image generator) polished it; he
  edited that result and art-directed corrections; ChatGPT polished those edits. The Mac tile and
  the simplified small-size drawings (the two SVGs) were then made for SilicaLayer with Claude to
  match it. It replaces LayerShop's icon in this fork. Licensed like the rest of the project, to
  the extent it is covered by copyright, under either of
  ```

- `ATTRIBUTION.md`, the `assets/app-icon/` row (about line 78): change only its third and fourth columns.
  - Third column becomes: `Erik Kristianson (crystal art: his drawing, refined in rounds with ChatGPT under his direction and edits); SilicaLayer contributors with Claude (Mac tile and small-size drawing)`
  - Fourth column becomes: `Human-drawn and AI-assisted, see [`assets/app-icon/LICENSE.txt`](assets/app-icon/LICENSE.txt)`
  - Leave the first, second and fifth columns as they are.

No tests to add; this is text only.

Edit only the files named above. Do not add files. Do not open image or binary files (nothing in `assets/app-icon/` except `LICENSE.txt`). Do not read `target/`.

Exit evidence: the full supervisor gate passes.
