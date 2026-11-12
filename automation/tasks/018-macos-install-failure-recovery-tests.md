# Synthetic failure-recovery tests for the macOS managed install

Test-coverage task; fix only what a test proves broken. V1 item 10 requires safe, actionable recovery from insufficient storage, network failure, corrupt downloads and engine damage on the reference Mac. Task 016 covers cancel, resume and restart; this task covers the failures. All tests go in `crates/aisetup/src/install/tests.rs`, reusing the macOS rig from `a_macos_managed_job_writes_its_platform_launches_without_windows_flags_and_skips_vcrt` (about line 604) and the tests 016 adds. Offline fixtures only; no network.

Tests (each must exercise the macOS `macos_arm64` path, `Choices { platform: runtime::RuntimePlatform::MacosArm64, variant: "macos_arm64".into(), test_image: false, ..rig.choices(&[]) }`):

- Not enough room: when the free-space check in `crates/aisetup/src/install.rs` (the `free < needed.saturating_add(plan::HEADROOM)` branch, about line 374) trips, the job fails with an `Error::Disk` whose message names the folder and the room needed, nothing is downloaded, and no marker is written. Use whatever seam the rig already offers for free space; if there is none, add the smallest injectable seam (a parameter or a test-only hook), not a real full disk.
- Network failure mid-download: the rig's server drops the connection part-way through the engine archive. The job stops with an actionable error (not a panic, not `Cancelled`), keeps the `.part` file, and a second run resumes and completes with the checksum verified.
- Corrupt finished archive: an archive whose bytes do not match the manifest's sha256 is rejected before unpacking, deleted or set aside, and re-downloaded on the next run.
- Damaged engine, then repair: after a completed install, change or delete a file inside the engine (for example `python/bin/python3` or `ComfyUI/main.py`), then run the job again with `recheck: true` (what `ai.setup.repair` does). The engine is restored to a working layout, the marker is valid, the executable bit on `python/bin/python3` is restored, and `<engineDir>/data` (task 013) is untouched.
- A repair never touches the models folder's files that are already verified (compare modification times or contents before and after).

If a test reveals a real bug, fix it in the smallest way in `crates/aisetup/src/install.rs`, `crates/aisetup/src/download.rs` or `crates/aisetup/src/unpack.rs`, and say so in a comment next to the test. Never weaken checksum, size, archive or permission checks. Edit no other files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.

## After the first run (review scope)

The tests and the repair fix in `crates/aisetup/src/install.rs` passed the gate. The review session then added an unrequested golden-test harness (`crates/aisetup/src/install/snapshot.rs`) that failed `rustfmt`; it has been removed. Review this task's diff only: do not add new files, modules, harnesses or tests beyond what this task lists. Known limitation, accepted for this task and queued separately: repair removes the marked old engine before unpacking, so a repair that fails mid-unpack leaves no engine until the next repair.
