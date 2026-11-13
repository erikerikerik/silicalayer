# Delete a set-aside engine only when it carries the app's marker

Small safety polish from the review of task 020. `remove_engines` in `crates/aisetup/src/runtime.rs` (about lines 362–395) deletes any folder in the engine directory whose name ends with `.replaced`, by name alone. A set-aside engine is always a moved engine folder, so it always carries this app's marker; a folder without one is not the app's and must stay. (`*.unpacking` staging folders have no marker and keep today's name-based rule.)

Scope:

- `crates/aisetup/src/runtime.rs`, `pub fn remove_engines`: remove a `*.replaced` folder only when `read_marker(&path).is_some()`. Leave the `*.unpacking` and marked-engine rules as they are.
- `crates/aisetup/src/platform.rs`, the doc comment on the `MacosArm64` variant (about line 18, "defined for future platform support; CPU mode is explicit"): it is supported now. Say it is the Apple Silicon managed runtime, qualified by unified memory (see `crates/aisetup/src/probe.rs`).

Tests (in `crates/aisetup/src/runtime.rs`'s test module, each must fail without the change):

- `remove_engines` removes a `0.41.0.replaced` folder that has the marker, and keeps a `notes.replaced` folder without one (with a file inside it).

Edit only `crates/aisetup/src/runtime.rs` and `crates/aisetup/src/platform.rs`. Do not add files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
