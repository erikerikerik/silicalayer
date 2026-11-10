# Restore declared runtime executables safely during unpack

Phase 2 runtime-finalization only. Make a future macOS ARM64 archive launchable after secure extraction by validating and applying a small manifest-declared executable allowlist, without enabling macOS installation or changing current Windows behavior.

Scope:

- Add a backward-compatible, default-empty `executables` list to each runtime `Asset`.
- Validate at manifest parse that the list is bounded to at most 128 unique plain relative paths, using the same hostile-input rules as runtime Python/script paths. Reject duplicates, empty paths, absolute paths, traversal, backslashes, drive/stream syntax, and malformed components.
- During managed-runtime unpack finalization, validate the Python file using `Layout::python` relative to `Layout::engine_root`, not the legacy global manifest path, so per-asset runtime layouts are honored.
- Before the staging folder becomes the engine, require every declared executable to exist as a regular file inside staging. On Unix, set exactly owner read/write/execute plus group/other read/execute (`0o755`). Keep Windows assets and manifests unchanged because their allowlists are empty.
- Apply permissions only to explicitly declared regular files after extraction; do not trust archive mode bits, follow symlinks, recurse, invoke a shell, or make undeclared files executable.
- A missing, non-file, or unmodifiable declared executable must fail the unpack atomically, leaving no completed engine or marker.
- Add focused synthetic tests for legacy empty-list behavior, valid executable paths, duplicates and hostile paths, successful Unix permission restoration, missing/non-file refusal, and per-asset Python validation.
- Do not add a macOS asset, URL, checksum, model, dependency, symlink support, install activation, UI change, download, release, signing, or public artifact.

Exit evidence: a future pinned ARM64 archive can declare exactly which files become executable, unpack finalization remains atomic and hostile-input-safe, Windows behavior is identical, and the full supervisor gate passes.
