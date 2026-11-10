# Version and validate the managed-runtime marker

Phase 2 runtime-boundary hardening only. Make the marker in `crates/aisetup/src/runtime.rs` an explicit, versioned proof that an engine directory is application-managed. Do not change installation, launch, removal, or model behavior.

Scope:

- Add a current marker format version and serialize it in newly written markers.
- Add a small typed platform field distinguishing the existing Windows portable runtime from the future macOS ARM64 managed runtime; reuse `RuntimePlatform` if practical without broad refactoring.
- Read the existing unversioned marker format as a legacy Windows portable marker so current installations remain discoverable.
- Reject unsupported future versions, malformed platform values, empty/default markers, empty ComfyUI versions, unknown/empty variants, and invalid SHA-256 strings as not being a trustworthy managed-runtime marker.
- Keep the existing 64 KiB marker size limit.
- Preserve the marker filename and the staging/write flow. Do not inspect or modify any real runtime directory during tests.
- Add synthetic tests for current round-trip, legacy migration, unsupported future version, malformed platform, empty marker, invalid digest, and oversized input.
- Update existing in-memory test fixtures and struct literals only as required by the typed marker.
- Do not change setup state, planning, downloads, archives, launch commands, removal policy, UI, manifests, dependencies, or any real on-disk runtime/model files.

Exit evidence: existing valid markers migrate predictably, new markers round-trip with explicit version and platform, hostile or ambiguous markers fail closed, and the full supervisor gate passes.
