# Enable the qualified Apple Silicon managed runtime

Phase 3 activation of the already-implemented macOS runtime boundaries.

Scope:

- Add the qualified, public, immutable Apple Silicon runtime archive to the bundled setup manifest with its measured size, SHA-256, unpacked size, longest path, runtime paths, and executable allowlist.
- Mark the existing `macos_arm64` runtime target as qualified and allow managed installation only on 64-bit Apple Silicon macOS hosts.
- Update the system check copy so supported Apple Silicon hosts pass and unsupported hosts still fail closed.
- Add focused regression tests for the exact bundled asset metadata and Apple Silicon install eligibility.
- Preserve Windows assets and behavior, external-ComfyUI behavior, model selection, download verification, hostile-archive checks, atomic extraction, and managed-runtime ownership boundaries.
- Do not add or change model weights, dependencies, protected branches, signing, app release behavior, paid APIs, or public application releases.

Exit evidence: the bundled manifest accepts the exact public runtime artifact, Apple Silicon probes select a qualified installable target, unsupported Macs remain blocked, and the repository gate passes.
