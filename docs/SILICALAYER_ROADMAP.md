# Roadmap and V1 Definition

This roadmap is deliberately port-first. It does not authorize a ground-up rewrite.

## Phase 0 — Establish the trustworthy baseline

- Review the LayerShop license, notices, third-party dependencies, model references, and branding constraints.
- Record the exact upstream commit used as the porting baseline.
- Create the SilicaLayer fork or import strategy without erasing upstream history.
- Build an inventory of Windows-only, NVIDIA-only, and already-portable code.
- Document current LayerShop behavior before changing it.
- Establish branch protections, CI permissions, secret handling, and contribution rules.

**Exit evidence:** baseline commit recorded, license questions identified, upstream application structure mapped, repository protections configured.

## Phase 1 — Qualify the Mac development host

Perform a read-only audit before installing anything:

- Exact macOS and M4 details
- Xcode and command-line tools
- Rust toolchain and target support
- Python availability without relying on system Python
- Free storage and filesystem format
- Code-signing identities without exposing certificate material
- Existing ComfyUI processes or installations
- Available build and test tooling

Then install only the pinned prerequisites needed for the baseline build.

**Exit evidence:** reproducible host report and successful build of the unmodified macOS-capable portions of upstream.

## Phase 2 — Refactor the setup boundary without changing behavior

- Isolate hardware probing, runtime planning, downloading, extraction, launching, health checking, and shutdown behind explicit interfaces.
- Preserve current Windows behavior while adding tests.
- Separate platform-independent wizard state from platform-specific runtime details.
- Add typed, versioned installation state and ownership records.

**Exit evidence:** existing behavior passes regression tests and macOS support can be added without duplicating the wizard.

## Phase 3 — Add the managed Apple Silicon runtime

- Implement macOS/ARM64 hardware and unified-memory probing.
- Define a pinned ARM64 Python/ComfyUI/PyTorch-MPS runtime manifest.
- Use application-owned paths under the appropriate macOS Library directories.
- Download with checksums, resume support, size limits, and safe extraction.
- Launch ComfyUI without Windows standalone arguments.
- Bind to loopback, supervise the owned process, and verify health.
- Never stop a server the application did not start.
- Add staged activation and rollback.

**Exit evidence:** a clean reference Mac can install, start, health-check, stop, repair, and update the private engine without Terminal use.

## Phase 4 — Qualify models and workflows

- Select one default model/workflow that fits 24 GB unified memory.
- Validate generation, editing/inpainting where supported, masks, cancellation, and repeated runs.
- Tune safe defaults for resolution, batch size, precision, sampler, and staged loading.
- Measure memory pressure, generation time, thermals, disk use, and failure recovery.
- Display model source, checksum, size, and license before installation.
- Confirm built-in ComfyUI nodes are sufficient for the V1 path.
- Evaluate MFLUX/MLX only if an essential workflow fails the ComfyUI/MPS qualification.

**Exit evidence:** repeatable output and recovery tests on the M4/24 GB reference Mac, with no unresolved crashes or silent corruption.

## Phase 5 — Complete the application experience

- Connect the managed runtime to the existing LayerShop setup wizard.
- Ensure generated results return as normal editable layers.
- Keep the existing editor and document behavior intact.
- Add actionable progress, cancellation, repair, and diagnostic views.
- Add optional external-ComfyUI configuration.
- Add optional cloud providers behind the existing provider boundary.
- Store provider credentials in Keychain.
- Apply SilicaLayer branding without redesigning the editor.

**Exit evidence:** a nontechnical designer can install, configure, generate, edit, save, reopen, and diagnose common failures without using Terminal.

## Phase 6 — Package and release securely

- Build a universal or ARM64 macOS application as decided by the support policy.
- Apply hardened runtime entitlements appropriate to the application.
- Sign the app and nested executable content.
- Notarize and staple the distributable.
- Build and test the DMG.
- Add a transactional, signed update mechanism with rollback.
- Run clean-install, upgrade, rollback, and uninstall tests.
- Publish checksums, provenance, licenses, and release notes.

**Exit evidence:** Gatekeeper accepts the release on a clean supported Mac and all release gates pass.

## Phase 7 — Public readiness

- Complete attribution and license notices.
- Document privacy and network behavior.
- Publish supported hardware and known limitations.
- Add contribution, security-reporting, and support guidance.
- Decide whether and how to propose portable changes upstream.
- Publish only after the security and release checklist is satisfied.

## V1 definition

V1 is complete only when all of the following are demonstrated on a clean, current macOS installation running on the M4 Mac mini with 24 GB unified memory:

1. The signed and notarized application installs without disabling macOS security.
2. First launch accurately checks hardware, memory, and storage.
3. The setup wizard installs a private, pinned ComfyUI/Python/MPS runtime without Terminal use.
4. Downloads are resumable and cryptographically verified.
5. A recommended model fitting the machine can be selected and installed with its license clearly shown.
6. The private engine binds to loopback, starts reliably, passes a health check, and shuts down only when owned by the app.
7. The user can generate an image through the normal LayerShop UI.
8. The result appears as an editable layer with the expected metadata and mask behavior.
9. Documents save and reopen correctly.
10. Cancellation, network failure, insufficient storage, corrupt downloads, engine failure, and memory pressure produce safe, actionable recovery.
11. Repair and update operations are staged, verified, and reversible.
12. Optional cloud generation requires explicit configuration and confirmation; it never becomes a silent paid fallback.
13. Logs and diagnostics contain no secrets.
14. Clean install, upgrade, rollback, and uninstall tests pass.
15. No known reproducible correctness or security defect remains in the V1 path.

A feature checklist alone is not proof of V1. Test records, release artifacts, and observed hardware behavior are required.
