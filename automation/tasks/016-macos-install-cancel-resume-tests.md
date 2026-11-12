# Synthetic cancel, resume and restart tests for the macOS managed install

Test-coverage task, no behaviour change expected. The macOS managed install has one end-to-end synthetic test (`a_macos_managed_job_writes_its_platform_launches_without_windows_flags_and_skips_vcrt`, about line 604 of `crates/aisetup/src/install/tests.rs`). Cancellation, resume and restart are only tested on the Windows path. V1 requires resumable, verified downloads and safe cancellation on the Mac.

Scope: add tests to `crates/aisetup/src/install/tests.rs` only, reusing that test's rig and its `macos_arm64` asset setup (`Choices { platform: runtime::RuntimePlatform::MacosArm64, variant: "macos_arm64".into(), test_image: false, ..rig.choices(&[]) }`). Use the rig's local, offline fixtures; no network.

- Cancel while the `macos_arm64` engine archive is downloading: the job ends `Paused` (see `Err(Error::Cancelled) => record(&env, &job, Phase::Paused, ...)` in `crates/aisetup/src/install.rs`, about line 283), a `<name>.part` file remains (`crates/aisetup/src/download.rs`), and no engine marker is written.
- Start the same job again: the download continues from the `.part` bytes (fewer bytes fetched than the full size, if the rig can count them), the checksum is verified, and the job completes with the macOS marker and launch arguments from the existing test.
- Cancel during unpack: no half-unpacked engine is left marked as installed (any `.unpacking` folder is not treated as an engine), and a restart completes cleanly.
- A corrupt `.part` (wrong bytes) is rejected by the checksum and re-downloaded, never unpacked.
- After a completed install, `<engineDir>/data/{user,input,output,temp}` exist and are outside the engine root (task 013).

If a test reveals a real bug, fix it in the smallest way in `crates/aisetup/src/install.rs` or `crates/aisetup/src/download.rs` and say so in a code comment next to the test. Do not weaken checksum, size or archive checks. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
