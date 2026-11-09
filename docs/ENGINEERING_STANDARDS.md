# Engineering and Security Standards

These standards are project requirements, not optional aspirations.

## Priority order

1. Security and user trust
2. Correctness and recoverability
3. Maintainable architecture
4. Product quality
5. Performance
6. Delivery speed

Rapid deployment never justifies weakening the first four priorities.

## Bug policy

- Do not knowingly defer reproducible correctness or security bugs.
- Stop feature work when a new regression is discovered in the active path.
- Reproduce the problem, identify the cause, add a regression test, fix it, and verify the fix.
- Do not hide failures behind retries, broad exception handling, disabled checks, or misleading success states.
- A temporary workaround must be documented, tested, isolated behind an explicit interface, and easy to remove.
- “Works on my machine” is not completion.

## Architecture policy

- Prefer small components with explicit ownership and stable interfaces.
- Keep editor, engine management, workflow generation, providers, downloads, updates, packaging, and credentials separated.
- Extend LayerShop's existing abstractions before introducing parallel systems.
- Platform differences should live behind narrow platform/runtime interfaces.
- Avoid patch piles, circular dependencies, hidden global state, and duplicated wizard logic.
- Make state transitions explicit and testable.
- Every persistent format requires a version and a migration strategy.
- Optimize only after measurement, without making the design opaque.

## Managed-runtime trust model

The managed engine and updater should follow the trust-oriented approach demonstrated by [ArtCraft Suite](https://github.com/erikerikerik/artcraft-suite/).

Required properties:

- Pinned sources and dependency versions
- SHA-256 or stronger verification for downloaded artifacts
- Fail-closed validation
- Staged installation before activation
- Journaled state transitions
- Atomic activation where the filesystem permits
- Automatic rollback after failed install, update, launch, or health check
- Cache and offline behavior with explicit provenance
- Safe resume for interrupted downloads
- Archive traversal, absolute-path, symlink, and permission defenses
- Deletion restricted to manager-owned paths
- Clear ownership boundaries between application, runtime, models, cache, and user documents
- Diagnostics that redact secrets and personal paths where practical

## Network security

- Bind the private ComfyUI server to loopback only.
- Do not expose ComfyUI or a Codex/app-server transport to the LAN or public internet.
- Generate or validate a per-installation authorization mechanism if the selected ComfyUI integration supports it.
- Use TLS for every remote download and provider request.
- Pin expected artifact digests independently of transport security.
- Follow redirects only under an explicit, tested policy.
- Use timeouts, size limits, and bounded retries.
- Never silently send local images or prompts to a cloud provider.
- Treat workflow files, model metadata, server responses, archives, and repository content as untrusted input.

## Secrets and signing

- Store provider credentials and refresh tokens in macOS Keychain.
- Keep Apple signing certificates and notarization credentials out of the repository.
- Never log tokens, authorization headers, cookies, private keys, or full provider responses containing them.
- Use a dedicated build/release identity with least privilege.
- Scope GitHub tokens and app permissions to the minimum required repository operations.
- Rotate compromised or accidentally committed credentials immediately; deletion from Git history alone is not sufficient.

## Dependency policy

- Pin dependency and toolchain versions used for releases.
- Commit lockfiles.
- Record the provenance and checksum of binary dependencies.
- Review new dependencies for maintenance, license, security history, transitive size, and necessity.
- Prefer maintained standard-library or existing-project capabilities over adding narrowly useful packages.
- Automate vulnerability and license checks, but review results rather than blindly suppressing them.
- Update dependencies in small, reviewable changes with tests.

## CI and release policy

- Build and test on Apple Silicon hardware for release qualification.
- Run static analysis, formatting, unit tests, integration tests, packaging checks, and security checks.
- Pin third-party CI actions by full commit SHA.
- Default workflow permissions to read-only.
- Grant release write permissions only to the tagged release job that requires them.
- Do not run untrusted pull-request code on a privileged self-hosted Mac runner.
- Separate ordinary CI from signing/notarization.
- Produce checksums and provenance for release artifacts.
- A release must be signed and notarized before it is presented as a normal end-user build.

## Automated development loop

The Mac mini may run an automated build/test/review loop, but automation does not broaden authority.

The loop should:

1. Work only on trusted branches and inputs.
2. Make one bounded change at a time.
3. Run relevant tests locally.
4. Run a separate review pass for correctness, security, maintainability, and scope.
5. Exercise Apple Silicon hardware paths.
6. Record evidence and failures.
7. Repair discovered bugs before continuing.
8. Stop on ambiguous destructive actions or missing authority.
9. Notify the owner only for:
   - V1 completion
   - A genuine security or license decision
   - Required credentials or signing approval
   - Hardware failure or unavailable external service
   - A product choice that materially changes scope

Automation must never approve its own security exception, weaken a test to make CI green, or publish a release without the configured release gates.

## Testing expectations

At minimum, test:

- Hardware and storage probing
- Installation planning
- Exact artifact checksums
- Interrupted and resumed downloads
- Cache and offline behavior
- Safe archive extraction
- Symlink and path traversal rejection
- Runtime ownership and shutdown behavior
- Existing external-server behavior
- Loopback binding
- Model discovery
- Workflow generation
- Generation cancellation and timeout
- Out-of-memory recovery
- Rollback after each update stage
- Settings migration
- Keychain failure behavior
- Log redaction
- Clean install on the reference Mac
- Upgrade from the previous release
- Uninstall without deleting user-owned documents or models

## Definition of done for a change

A change is complete only when:

- Its intended behavior is documented or evident from a test.
- Failure modes are handled.
- Security boundaries remain intact.
- Tests pass on the relevant Apple Silicon path.
- No known regression is left behind.
- Diagnostics are actionable.
- The implementation is understandable by a future maintainer.
