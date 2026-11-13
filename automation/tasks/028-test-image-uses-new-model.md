# The setup's test image tests the model just added, and only when there is something new to test

Bug found in hands-on testing. After a setup run, the wizard renders a test image (`"testImage": true` by default, `crates/engine/src/ai_setup_cmds.rs` about line 479). The model comes from `pub fn test_model(settings, ready)` in `crates/aisetup/src/configure.rs`: the first enabled local model in the settings list that is ready. A user who already has Z-Image Turbo and adds MOSS-Image therefore waits for another Z-Image Turbo render (about 7 minutes on the M4), and MOSS-Image is never tested.

Scope:

- `crates/aisetup/src/configure.rs`, `test_model`: take the ids of the models this run newly made ready (downloaded or newly configured) as well. Prefer the first of those that can generate (`m.can_generate()`), in settings order. If the run added no new model and the engine itself was not installed, replaced or repaired in this run, return `None`, so no test image is rendered.
- `crates/aisetup/src/install.rs`: where the run calls `test_model` (find the call), pass the newly ready ids. The run already knows which models it downloaded or set up (its steps and the record before/after); compute "new" as ready now and not installed before this run. A repair (`recheck: true`, engine re-unpacked) still renders a test image with the first ready model, as today.
- A run with the CPU option keeps today's "no test image" rule.
- Nothing else about the test image changes (its size, how its time is reported, `finished_value`'s `testImage`).

Tests (each must fail without the change), in `crates/aisetup/src/configure.rs`'s test module and `crates/aisetup/src/install/tests.rs` (reuse the existing rigs):

- `test_model` with Z-Image Turbo already ready and MOSS-Image newly ready picks MOSS-Image.
- With nothing new and the engine unchanged, it returns `None`, and the install run renders no test image.
- A first install (engine new) with only Z-Image Turbo still tests Z-Image Turbo.
- A repair still renders a test image.

Edit only `crates/aisetup/src/configure.rs`, `crates/aisetup/src/install.rs` and `crates/aisetup/src/install/tests.rs`. Do not add files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
