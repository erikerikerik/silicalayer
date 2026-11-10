# Add per-asset runtime paths without activating macOS installation

Phase 2 manifest-boundary refactoring only. Let a runtime asset describe the Python executable and ComfyUI entry script inside its own archive, while keeping the bundled Windows manifest and every current Windows install/launch behavior unchanged.

Scope:

- Add optional `python` and `script` paths to each runtime `Asset`; absent values must fall back to the existing `Comfy.python` and `Comfy.script` fields so manifest format version 1 and the bundled manifest remain backward compatible.
- Add one typed manifest accessor that resolves an asset variant to its validated runtime paths. Require an asset override to provide both paths or neither; reject a one-sided override.
- Apply the same hostile-input rules to asset-specific paths that protect the current global paths: non-empty, plain forward-slash-separated relative paths, with no empty, dot, parent, backslash, drive, or absolute components.
- Make `Layout::new` use the resolved paths for its `python` and `script` fields.
- Make launch construction use `Layout::script`, not the global manifest script, so a future `macos_arm64` asset can actually launch its own archive layout.
- Preserve the bundled `manifest.json` byte-for-byte and preserve all current Windows layout and command-line results exactly.
- Add focused synthetic tests for legacy fallback, a complete macOS-style override, missing variant, one-sided overrides, absolute/traversal/Windows-drive/backslash paths, and layout/launch use of the override.
- Do not add a macOS asset URL or checksum, change manifest version, enable macOS installation, download or unpack anything, change archive permission handling, change UI, change model selection, change dependencies, or touch external-ComfyUI behavior.

Exit evidence: runtime paths are selected per asset through one validated boundary, legacy Windows manifests behave identically, a synthetic `macos_arm64` layout reaches launch construction, and the full supervisor gate passes.
