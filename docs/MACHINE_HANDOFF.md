# Machine Handoff and Working Notes

This repository is the durable source of truth for moving SilicaLayer work between the original laptop and the M4 Mac mini.

## Current situation

The planning conversation began in a local, projectless Codex workspace. That folder was not a Git repository, so the original chat could not be handed directly to another Codex host using Git-state handoff.

The decisions from that conversation are captured in this repository:

- [README](../README.md)
- [Project brief](PROJECT_BRIEF.md)
- [Engineering standards](ENGINEERING_STANDARDS.md)
- [Roadmap](ROADMAP.md)

Future implementation conversations should be opened against a real checkout of this repository.

## Recommended host setup

On both machines:

1. Install the current ChatGPT desktop app.
2. Sign in with the same authorized account and workspace.
3. Clone this repository.
4. Save the checkout as a Codex project.
5. Use a dedicated branch or Codex-managed worktree for implementation.
6. Keep credentials and machine-specific configuration outside Git.

For remote control or host handoff:

1. On the Mac mini, open **Settings → Connections → Control this Mac**.
2. On the controlling laptop, open **Settings → Connections → Control other devices**.
3. Pair the machines through the supported authenticated flow.
4. Use the chat footer's run-location control to hand off an eligible project chat.
5. Verify the destination repository and branch before confirming.

Do not expose a Codex app-server transport, SSH service, or ComfyUI port directly to the public internet. Use the product's supported secure connection flow; if ordinary SSH is later required outside the LAN, use authenticated keys and a trusted VPN or mesh network.

Reference: [OpenAI Remote Connections documentation](https://learn.chatgpt.com/docs/remote-connections)

## Mac mini role

The M4 Mac mini is the reference development, test, signing, and release machine.

Expected responsibilities:

- Build the Rust/macOS application
- Install and test the private ARM64 ComfyUI runtime
- Run MPS inference qualification
- Execute hardware-specific integration and regression tests
- Package, sign, notarize, and verify releases
- Run the bounded automated build/test/review loop
- Retain local release evidence and diagnostics

The 4090 PC is not part of the initial build plan. It may be revisited later for unrelated projects, additional model evaluation, or cross-platform CI, but V1 must not depend on it.

## Storage layout guidance

Internal storage:

- macOS, Xcode, and command-line tools
- Active repository and worktrees
- Active build directory
- Managed runtime
- Default qualified model
- Signing/notarization staging
- Swap and operational reserve

Crucial X10 Pro external SSD:

- Additional models
- Archived build artifacts
- Download cache
- Test corpus backups
- Historical diagnostics
- Recoverable local mirrors

Maintain approximately 70–80 GB free internally.

## Secrets that do not move through Git

- Apple signing certificate private keys
- Notarization credentials
- GitHub access tokens
- Cloud-model API keys
- Keychain databases
- SSH private keys
- Personal test images
- Downloaded model weights unless their distribution license explicitly permits it
- Generated diagnostics that have not been reviewed for sensitive data

Provision secrets independently on the Mac mini using Keychain or the relevant secure credential store.

## Starting a fresh implementation chat

Use a prompt similar to:

> Work from the SilicaLayer repository documentation as the governing project requirements. Begin with Phase 0 and a read-only audit. This is a port of LayerShop to Apple Silicon, not a ground-up rewrite. Do not install dependencies, modify source, or create external resources until the audit is reported. Security, correctness, recoverability, and maintainability outrank delivery speed.

The agent should read all files in `docs/` before proposing implementation changes.

## Decision log

### Confirmed

- Port LayerShop rather than rebuild it.
- Retain the existing editor and UI.
- Use a private managed ComfyUI engine by default.
- Support local inference first and optional cloud providers.
- Fit and qualify at least one useful model on 24 GB unified memory.
- Treat MLX/MFLUX as a fallback only.
- Develop and test entirely on the M4 Mac mini initially.
- Prefer internal storage for active work; use the external SSD as secondary capacity.
- Fully automate repeatable build/test/review work within strict security boundaries.
- Fix reproducible bugs as they appear.
- Publish useful work only after license, attribution, security, and release requirements are satisfied.
- Use SilicaLayer as the working project identity.

### Still to decide after audit

- Exact upstream commit or fork/import strategy
- Minimum supported macOS version
- ARM64-only versus universal application release
- Exact Python, PyTorch, and ComfyUI versions
- Default model and quantization
- Model storage customization policy
- Cloud providers included in V1
- Updater technology
- Final bundle identifier and signing configuration
- Public release license for original SilicaLayer contributions
