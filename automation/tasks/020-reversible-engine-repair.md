# Make engine repair reversible: keep the old engine until the new one is in place

V1 item 11 (repair and update are staged, verified and reversible). Since task 018, `unpack_step` in `crates/aisetup/src/install.rs` (about lines 476–483) deletes an engine folder that carries this app's marker before calling `unpack::unpack`, because `unpack` (in `crates/aisetup/src/unpack.rs`, about lines 220–232) refuses a non-empty destination. If the unpack then fails or is cancelled, the user is left with no engine at all.

Scope:

- In `crates/aisetup/src/install.rs`, instead of `remove_dir_all(&layout.engine_root)`, rename the marked engine to a sibling `<version>.replaced` folder (remove a stale `.replaced` first if one exists). Then call `unpack` on the now-absent destination as today.
- If `unpack` succeeds, delete `<version>.replaced`. If it fails or is cancelled, rename `<version>.replaced` back to `layout.engine_root` (only if `engine_root` does not exist), then return the original error.
- `crates/aisetup/src/runtime.rs`, `installed_engines` and `remove_engines` (about lines 350–395): a `.replaced` folder is not an installed engine, but `remove_engines` removes it like `.unpacking`.
- On start-up of a job, a leftover `<version>.replaced` next to a missing `engine_root` (a crash mid-repair) is renamed back before anything else; next to a present `engine_root` it is deleted.
- Never touch `<engineDir>/data`, and never delete a folder without this app's marker.

Tests (in `crates/aisetup/src/install/tests.rs`, reusing the macOS rig and the 016/018 helpers; each must fail without the change):

- Repair of a damaged engine whose unpack is cancelled: afterwards the original (damaged) engine folder is back in place with its marker, and no `.replaced` remains.
- Repair whose archive is corrupt: same outcome, and the error is the archive error.
- Successful repair: the new engine is in place, `.replaced` is gone, `<engineDir>/data` is untouched.
- A leftover `.replaced` with no `engine_root` is restored at the next job's start; with an `engine_root` it is deleted.

Edit only `crates/aisetup/src/install.rs`, `crates/aisetup/src/runtime.rs` and their existing test modules. Do not add files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
