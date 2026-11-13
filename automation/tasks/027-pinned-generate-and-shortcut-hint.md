# Keep Generate visible and show the keyboard shortcut

End-user fix from hands-on testing. In the AI Gen panel (`crates/ui-egui/src/ai_gen_ui.rs`) the prompt box is multiline (`egui::TextEdit::multiline(&mut app.ui.ai_gen.prompt)`, about line 1858), so Return adds a line; Command-Return generates (about line 1885) but nothing says so. The Generate button (about line 2002) is at the bottom of the scrolling panel, so in a short panel it, and the Advanced section's Seed row, scroll out of sight and the panel looks broken.

Scope:

- Pin the row with the Generate button to the bottom of the AI Gen panel, outside its scroll area, so it is visible whatever the panel's height. Everything else keeps scrolling. Keep the button's enabled rules and tooltip as they are.
- Under the prompt box, show a small dim hint: "⌘↩ to generate" on macOS and "Ctrl+Enter to generate" elsewhere (match what `modifiers.command` means on each platform).
- Plain Return keeps adding a new line.

Tests (each must fail without the change), in `crates/ui-egui/src/ai_gen_ui/tests.rs`:

- With the panel laid out 260 px tall and Advanced open, the Generate button's rect is inside the panel's visible rect.
- The hint text is drawn under the prompt box, with the platform's wording.

Edit only `crates/ui-egui/src/ai_gen_ui.rs` and `crates/ui-egui/src/ai_gen_ui/tests.rs`. Do not add files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
