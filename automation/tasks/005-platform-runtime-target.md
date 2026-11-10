# Add a typed platform runtime target to the host probe

Phase 2 selection-boundary refactoring only. Connect the existing macOS host facts to the existing `RuntimePlatform` type through a pure, testable probe result, without enabling macOS installation or changing the current Windows wizard behavior.

Scope:

- Put `RuntimePlatform` in a module available to both the native runtime code and the cross-platform probe code, or introduce an equivalent shared type and keep callers explicit.
- Add a small typed probe result that carries runtime platform, asset variant, and whether that host/runtime pairing is qualified.
- For supported 64-bit Windows hosts, preserve the existing NVIDIA/AMD/Intel variant selection and qualification values exactly.
- For 64-bit macOS hosts with a non-empty Apple Silicon identity, select `MacosArm64` and the existing marker variant `macos_arm64`; keep it unqualified until the MPS runtime/model spike supplies evidence.
- Unsupported, 32-bit, Intel-macOS, malformed, and incomplete synthetic hosts must produce no managed-runtime target.
- Keep the existing `variant()` API and every current Windows caller behaving as before; it may delegate internally to the new boundary.
- Keep `can_install()` and the first-screen support/stop messages unchanged. This task must not activate macOS installation before a pinned runtime asset exists.
- Add focused synthetic tests for qualified modern NVIDIA Windows, older-driver Windows, AMD/Intel Windows, Apple Silicon macOS, Intel macOS, 32-bit hosts, and malformed/incomplete host facts.
- Do not change manifests, downloads, archives, unpacking, process launch, UI text, model selection, dependencies, external-ComfyUI behavior, or any real files outside the test process.

Exit evidence: the probe exposes one conservative typed runtime target, all legacy Windows behavior remains green, macOS selection is represented but not installable, and the full supervisor gate passes.
