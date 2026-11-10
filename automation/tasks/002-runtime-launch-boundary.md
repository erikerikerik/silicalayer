# Extract a typed managed-runtime launch boundary

Phase 2 refactoring only. Make the managed ComfyUI launch command in `crates/aisetup/src/runtime.rs` explicitly platform-aware without changing any current caller or Windows behavior.

Scope:

- Introduce a small typed runtime-platform or launch-flavor value with Windows portable and macOS ARM64 variants.
- Move command construction into a pure function that accepts that value, the manifest/layout, and the CPU choice.
- Keep the existing public `launch(...)` function and all current callers behaving exactly as Windows portable does today.
- Define and unit-test the future macOS command shape: the managed Python runs the pinned ComfyUI script with `-s`, `--disable-auto-launch`, and `--extra-model-paths-config`, but never `--windows-standalone-build`; CPU mode remains explicit.
- Preserve argument quoting through `join_args`/`split_args` and paths containing spaces.
- Add no runtime downloads, manifests, dependencies, process launch, UI changes, branding, or activation of macOS installation.
- Do not change model discovery, removal, health checks, shutdown, Windows installation, or external-ComfyUI behavior.

Exit evidence: focused synthetic tests cover both command shapes, the existing Windows test remains green, and the full supervisor gate passes.
