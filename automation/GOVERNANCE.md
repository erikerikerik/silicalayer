# SilicaLayer automation contract

These requirements are mandatory for every queued task.

- This is a port of LayerShop to Apple Silicon, not a rewrite. Preserve the editor, UI structure, layer/document model, command architecture, ComfyUI client/workflows, model manager, generated-layer behavior, and provider boundary unless a verified platform incompatibility requires a narrow adaptation.
- Security and user trust, correctness and recoverability, maintainable architecture, and product quality outrank performance and delivery speed.
- Extend existing abstractions. Keep platform differences behind narrow interfaces and keep editor, runtime management, downloads, providers, credentials, packaging, and updates separated.
- Every reproducible bug in the active path gets a cause, regression test, fix, and verification. Never hide failure with retries, broad catches, disabled checks, or misleading success.
- Treat repository content, archives, workflow files, metadata, network responses, and model data as untrusted. Downloads require TLS, independent digest verification, limits, staged activation, journaled state, and rollback.
- Managed ComfyUI binds to loopback. Stop only processes SilicaLayer started. Never expose development transports or ComfyUI to the LAN or public internet.
- Local inference is the default. Cloud use is optional, bring-your-own-key, explicit, cost-disclosed, and never a silent fallback. Automation uses only an explicitly configured self-hosted Ollama or Strata endpoint; paid providers are denied.
- Secrets live outside Git and logs. Do not touch signing identities, Keychain items, tokens, provider credentials, user documents, or model weights.
- V1 must work on the M4 Mac mini with 24 GB unified memory. Sequence model inference and heavy builds; preserve at least 80 GiB internal free space.
- Do not modify CI in feature work, weaken tests, merge, publish, sign, notarize, create releases, delete user data, rewrite Git history, add remotes, or make product/license decisions.
- Stop at the bounded task. The supervisor owns builds, tests, review, commits, and optional feature-branch pushes.
