# Propagate the managed runtime platform through installation

Phase 2/3 boundary refactoring only. Carry the already-defined `RuntimePlatform` through a managed setup choice so a synthetic macOS ARM64 install writes the right ownership marker, launches without Windows-only arguments, and skips Windows-only checks, while bundled Windows behavior remains byte-for-byte compatible at its external boundaries.

Scope:

- Add an explicit, serde-defaulted runtime platform to managed `Choices`. Old choices JSON that omits it must deserialize as `windowsPortable`; existing constructors and Windows tests must continue to behave exactly as before.
- Centralize the valid variant/platform pairing in the typed platform boundary: Windows portable accepts only `nvidia`, `nvidia_cu126`, `amd`, and `intel`; macOS ARM64 accepts only `macos_arm64`.
- Refuse a managed plan/layout whose explicit platform and variant do not match before any download, extraction, marker write, process launch, or disk mutation. Existing-route choices must remain unaffected because they do not install a runtime.
- Use the choice's platform when writing the managed-runtime marker. Do not infer it from the host OS, archive contents, path names, or variant string outside the typed validation boundary.
- Use the choice's platform when constructing the managed launch command. Preserve every current Windows argument; macOS ARM64 must omit `--windows-standalone-build` and retain loopback port injection by the existing launcher.
- Run the Visual C++ runtime repair and the Windows maximum-path preflight only for `windowsPortable`; never copy Windows DLLs or apply the Windows path limit to a macOS ARM64 choice.
- Add focused synthetic tests for old JSON defaults, valid and mismatched platform/variant pairs, correct Windows and macOS marker platforms, distinct launch arguments, and Windows-only VCRT/path behavior. Tests must not depend on the executing host OS.
- Keep `Probe::can_install()` false for macOS. Do not add a bundled macOS asset, URL, checksum, archive, model, dependency, download, UI activation, release, signing, network call, or public artifact.

Exit evidence: a synthetic `macosArm64` managed job reaches the existing platform-correct marker and launch boundaries without Windows-only behavior, invalid pairs fail closed before mutation, legacy Windows choices and installs are unchanged, and the full supervisor gate passes.
