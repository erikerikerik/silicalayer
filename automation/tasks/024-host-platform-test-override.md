# Let tests choose the host platform, so Windows behaviour is tested on the Mac

Test-robustness polish. Since tasks 011, 015 and 022, setup behaviour depends on `RuntimePlatform::host()` (`crates/aisetup/src/platform.rs`), and the setup tests pick their expectations with `#[cfg(all(target_os = "macos", target_arch = "aarch64"))]`. The supervisor gate runs only on the Mac, so the Windows branches of those tests never run here, and a change that breaks Windows behaviour would pass the gate.

Scope:

- `crates/aisetup/src/platform.rs`: add a test-only override for `host()`. A `#[cfg(any(test, feature = "test-host"))]`-style feature is not needed; use a `thread_local!` `Cell<Option<Option<RuntimePlatform>>>` that `host()` consults first, set through a guard type, e.g. `pub fn override_host_for_tests(host: Option<RuntimePlatform>) -> HostOverride` whose `Drop` restores the previous value. It must be available to the engine and UI crates' tests (so not `#[cfg(test)]` inside `aisetup`); keep it `#[doc(hidden)]` and document that production code never calls it. Production behaviour of `host()` is unchanged when no override is set.
- Tests in `crates/engine/src/ai_setup_cmds/tests.rs` and `crates/ui-egui/src/ai_setup_ui/tests.rs` that branch on `cfg(target_os = "macos")` for host-dependent expectations: run each such scenario for both a `WindowsPortable` host and a `MacosArm64` host through the override, asserting the respective expectations, instead of choosing one branch at compile time. Keep every existing assertion's meaning.
- Only tests that run on one thread may use the override (the setup UI tests already drive the app synchronously). If a test spawns threads that call `host()`, leave that test as it is.

Tests:

- `crates/aisetup/src/platform.rs`: `host()` returns the override inside the guard's scope and the real value after it is dropped, including when nested.
- The converted engine and UI tests pass for both hosts on the Mac.

Edit only `crates/aisetup/src/platform.rs`, `crates/engine/src/ai_setup_cmds/tests.rs` and `crates/ui-egui/src/ai_setup_ui/tests.rs`. Do not add files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
