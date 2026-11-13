# Auto appearance ignores a system light/dark reading that lasts under a second

Erik's report: after locking and unlocking the Mac, SilicaLayer's grey interface flickered between light grey and dark grey for a moment, then settled. macOS is set to Dark. It could not be reproduced on demand. Cause: with Appearance on Auto, `sync_appearance` (`crates/ui-egui/src/prefs/prefs_ui/appearance.rs`, about line 189) runs every frame from `tick` (`prefs_ui/save.rs`, line 13) and switches theme the instant the system reading changes. During lock and unlock macOS can briefly report Light, so the app flips to the light theme and back. Fix: a new system reading only takes effect once it has held for 1 second. Changes the user makes (Preferences, the Window › Theme menu, darken/lighten) still apply at once.

Scope:

- `crates/ui-egui/src/state.rs`, `UiState` (about line 460, next to `pub theme`), and its default (about line 648): add
  - `pub system_theme_accepted: Option<Option<egui::Theme>>` (outer `None` = no reading taken yet), default `None`;
  - `pub system_theme_pending: Option<(Option<egui::Theme>, f64)>` (a new reading and the time it was first seen), default `None`.
  Use small targeted edits in this file.

- `crates/ui-egui/src/prefs/prefs_ui/appearance.rs`:
  - Add a pure function
    `pub(super) fn settle_system_theme(accepted: &mut Option<Option<egui::Theme>>, pending: &mut Option<(Option<egui::Theme>, f64)>, reading: Option<egui::Theme>, now: f64) -> (Option<egui::Theme>, Option<f64>)`
    returning the reading to use and, if a change is waiting, the seconds left before it can be accepted. Rules:
    1. If `accepted` is `None` (first frame), accept `reading` at once, clear `pending`, return `(reading, None)`.
    2. If `reading` equals the accepted reading, clear `pending` and return it with `None`.
    3. Otherwise, if `pending` holds the same reading and `now - since >= SYSTEM_THEME_SETTLE` (a new `const SYSTEM_THEME_SETTLE: f64 = 1.0;`), accept it, clear `pending`, return `(reading, None)`.
    4. Otherwise, if `pending` is empty or holds a different reading, set `pending = Some((reading, now))`. Return the old accepted reading and `Some(seconds left)`.
  - In `sync_appearance`, replace `system_theme(app, ctx)` with the settled reading: take `now = ctx.input(|i| i.time)`, call `settle_system_theme` with the two `app.ui` fields, and when it returns `Some(left)`, call `ctx.request_repaint_after(std::time::Duration::from_secs_f64(left))` so the check runs again without user input. Keep the rest of `sync_appearance` as it is. Add a short doc comment saying why.
  - Leave `system_theme()` itself unchanged (the Preferences swatches still show the live reading).

- `crates/ui-egui/src/prefs/prefs_ui/tests.rs`: add tests of `settle_system_theme`:
  - first reading is accepted at once;
  - a Light reading while Dark is accepted is ignored at 0.0 s and 0.9 s, and accepted at 1.0 s;
  - Dark → Light (0.5 s) → Dark keeps Dark and clears `pending`;
  - Dark → `None` for 0.5 s → Dark never changes the returned reading;
  - the returned seconds left count down (1.0 at first sight, about 0.6 after 0.4 s).
  Existing tests that call `sync_appearance` must still pass. If one of them changes the system reading between two calls and expects an instant switch, update it to advance the input time by at least 1 second (or set `app.ui.system_theme_accepted = None` before the call), and say which in the commit.

Edit only the files named above. Do not add files. Do not open image or binary files. Do not read `target/`.

Exit evidence: the full supervisor gate passes.
