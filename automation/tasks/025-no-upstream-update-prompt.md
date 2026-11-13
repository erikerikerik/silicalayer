# Never offer upstream LayerShop's releases as updates

Correctness and privacy fix found in hands-on testing. The fork still checks upstream's release feed: `apps/photocraft/src/updates.rs` has `pub const FEED: &str = "https://api.github.com/repos/SlavomirDurej/layershop/releases?per_page=30";` and `pub const RELEASES` pointing at the same upstream repo. A SilicaLayer build therefore contacts that third-party repository at launch and shows "LayerShop by Slavomir Durej — v0.21.0 to 0.23.1 — Download", offering a different author's app that lacks this port. SilicaLayer has no public release feed yet (the app repo is private; a signed updater is Phase 6 work), so there must be no update check at all for now.

Scope:

- `apps/photocraft/src/services.rs`, where the services are built (`check_updates: Some(crate::updates::service()),`, about line 771): set `check_updates: None`, with a comment saying SilicaLayer has no public release feed yet and upstream's must never be offered; Phase 6 adds a signed updater against SilicaLayer's own releases.
- Make sure nothing else in the app contacts `api.github.com/repos/SlavomirDurej/layershop` or `storytold/photocraft` for updates (search `apps/photocraft/src` and `crates/ui-egui/src` for those strings outside tests).
- `crates/ui-egui/src/updates_ui.rs`: with no check service (`session.updates.check` is `None`, see `crates/ui-egui/src/app.rs` about line 38), no update dialog, badge or "Check for Updates" action is shown, and the automatic check at about line 98 does nothing. If the menu item or preference toggle is drawn unconditionally today, hide it when there is no check service. Keep the update code and its tests for Phase 6.
- If `apps/photocraft/src/updates.rs` becomes unused, keep it compiling without warnings (the gate denies Clippy warnings): for example keep `service()` referenced from its own tests, or gate the unused items with a narrowly scoped `#[allow(dead_code)]` and a comment pointing at Phase 6. Do not delete the module.

Tests (each must fail without the change):

- `crates/ui-egui/src/updates_ui.rs` tests: with no check service and `automatically_check_updates` true, drawing the app for several frames starts no check, shows no update prompt, and offers no "Check for Updates" action.
- An `apps/photocraft` test (in `apps/photocraft/src/services.rs` or `updates.rs`) that the services the app builds have `check_updates` set to `None`.

Edit only `apps/photocraft/src/services.rs`, `apps/photocraft/src/updates.rs`, `crates/ui-egui/src/updates_ui.rs` and `crates/ui-egui/src/app.rs`. Do not add files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
