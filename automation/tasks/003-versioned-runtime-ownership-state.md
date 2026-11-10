# Add versioned runtime ownership to setup state

Phase 2 state refactoring only. Extend `crates/aisetup/src/state.rs` with a typed, versioned record of who owns the ComfyUI runtime, without changing installation behavior or file deletion.

Scope:

- Add a current setup-state format version and serialize it in new records.
- Add a small typed runtime-ownership enum that distinguishes an application-managed private runtime from the user's external runtime; include a conservative unknown/default value if needed for malformed input.
- Migrate legacy records with no version in memory: `Route::Managed` becomes application-managed and `Route::Existing` becomes user-owned/external.
- Preserve every existing field and the rule that transient `server_files` is never persisted.
- Reject records with an unsupported future format version as no usable record rather than guessing or rewriting their meaning.
- Keep the one-megabyte input limit and all existing string/vector bounds.
- Add synthetic tests for a legacy managed record, a legacy external record, current-version round-trip, malformed ownership, and an unsupported future version.
- Keep atomic save behavior unchanged.
- Do not change planning, downloading, unpacking, launching, removal, UI, manifests, dependencies, or any on-disk runtime/model files.

Exit evidence: old unversioned records migrate predictably, new records round-trip with explicit version and ownership, hostile/future records fail closed, and the full supervisor gate passes.
