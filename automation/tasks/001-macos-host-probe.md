# Add a tested macOS host probe boundary

Phase 2 preparation only. Extend the existing `photocraft-aisetup` probing abstraction so macOS host facts needed by a future managed ARM64 runtime can be represented and tested without changing Windows behavior or installing a runtime.

Scope:

- Represent Apple Silicon architecture, unified memory, and free storage through the existing probe boundary.
- Keep platform-specific collection narrow and testable.
- Add synthetic unit tests for supported and malformed inputs.
- Do not add downloads, Python/ComfyUI installation, process launch, UI redesign, branding, cloud behavior, or dependencies.
- Preserve existing Windows and external-ComfyUI behavior.
