# Keep the managed engine out of Time Machine and Spotlight on macOS

macOS polish with a real cost behind it. The managed engine unpacks to about 1.9 GB of Python and ComfyUI files under `~/Library/Application Support/<app>/LocalAI/engine/<version>` (`apps/photocraft/src/app_dirs.rs`, `fn local_ai_dirs`). Time Machine backs it up on every change and Spotlight indexes tens of thousands of files, although the engine is reproducible from the pinned, checksummed archive.

Scope (macOS only, `#[cfg(target_os = "macos")]` or a runtime platform check; Windows and other platforms unchanged):

- In `crates/aisetup/src/install.rs`, after a successful unpack of a managed engine (the `Ok(_)` arm after `unpack::unpack` in `fn unpack_step`), mark `layout.engine_root` as excluded from backups and indexing:
  - Run `tmutil addexclusion <engine_root>` through `std::process::Command` (no `-p`; stdin/stdout/stderr null). A failure never fails the install.
  - Create an empty `.metadata_never_index` file inside `engine_root` (ignore errors).
- Put the logic in a small helper in `crates/aisetup/src/install.rs` (for example `fn exclude_from_backup_and_index(dir: &Path, run: &dyn Fn(&str, &[&str]) -> bool)`) so a test can check the exact `tmutil` arguments without running `tmutil`.
- Do not mark the user's models folder or `<engineDir>/data`. If engine verification would treat `.metadata_never_index` as an unexpected file, make it ignore that name.

Tests (each must fail without the change), in `crates/aisetup/src/install/tests.rs`:

- With a fake runner, a completed macOS managed install calls `tmutil addexclusion <engine_root>` exactly once and writes `<engine_root>/.metadata_never_index`.
- A failing fake runner does not fail the install.
- The models folder and `<engineDir>/data` get neither the exclusion call nor the marker file.
- A Windows managed install (existing rig) makes no such call.

Edit only `crates/aisetup/src/install.rs` and `crates/aisetup/src/install/tests.rs`. Do not add files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
