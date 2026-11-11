# Expose the typed runtime target through setup commands

Phase 3 command-boundary wiring only. Surface the host probe's already-defined `RuntimeTarget` and carry an explicitly requested runtime platform into managed choices, without enabling macOS installation or adding an archive.

Scope:

- Make `ai.setup.probe` return the typed runtime target (`platform`, `variant`, `qualified`) selected by `Probe::runtime_target()`. Preserve the legacy `variant` and `variantTested` fields for Windows/UI compatibility, deriving them from the same target when one exists.
- Accept an optional managed `platform` parameter in setup plan/start choice parsing, with exactly the serialized values `windowsPortable` and `macosArm64`. When absent, preserve the saved choice; old/empty records remain Windows portable.
- Return the selected platform from plan and status records, and include marker platform in installed-engine listings. Existing records and external-ComfyUI routes remain backward compatible.
- Rely on the Task 8 platform/variant validation and the manifest lookup to fail closed. The bundled manifest still has no `macos_arm64` asset, so no Mac runtime download can begin.
- Update command parameter/result documentation and add focused command tests for probe target output, old defaults, explicit platform parsing, invalid values, status round-trip, and rejection of a Mac platform paired with the bundled Windows assets.
- Do not change `Probe::can_install()`, wizard screens, the bundled manifest, archive URLs/checksums, downloads, UI activation, models, dependencies, release/signing, network behavior, or external-ComfyUI behavior.

Exit evidence: the command boundary reports and preserves an explicit typed runtime platform without inference, invalid and unavailable pairs fail before mutation, all Windows behavior remains compatible, and the full supervisor gate passes.
