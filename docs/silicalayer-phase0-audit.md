# SilicaLayer Phase 0 audit

Audit date: 2026-11-09

Upstream: `SlavomirDurej/layershop`

Baseline: `5d3cff0b210ff41999e5a0dc5747683d52383ea1` (`layershop-v0.21.0-preview.1`)

## Baseline and import strategy

LayerShop is itself a history-preserving fork of PhotoCraft. SilicaLayer should therefore retain the complete LayerShop Git history and add the private SilicaLayer repository as the destination remote. Do not copy a source archive into the planning repository or squash the upstream baseline.

The working checkout used for this audit was created from the complete LayerShop history, with `upstream` pointing to the public LayerShop repository and `origin` reserved for `erikerikerik/silicalayer`. The private repository's planning commit must be merged into this lineage once a repository-scoped local Git credential is available. Resolve the README and documentation layout explicitly; do not discard either history.

## License and attribution

- LayerShop is offered under `MIT OR Apache-2.0` and includes both license texts.
- `NOTICE` and `ATTRIBUTION.md` are material distribution inputs and must remain intact.
- Bundled fonts, icons, images, dictionaries, and third-party code have per-asset attribution requirements.
- ArtCraft names and logos are excluded trademarks. Adobe, Photoshop, ComfyUI, and LayerShop names require accurate compatibility/relationship wording.
- Model weights have separate licenses. Several model presets are non-commercial or gated; no model may be bundled or presented for commercial use without a recorded license decision.
- The public license for new SilicaLayer-only contributions remains undecided. No public release is authorized by this audit.

## Architecture map

- L0–L3: geometry, color, raster, document, operations, composition, GPU, and native format. These are largely platform-neutral Rust.
- L4: `photocraft-aigen` owns ComfyUI workflows and provider boundaries; `photocraft-aisetup` owns probing, planning, downloads, extraction, runtime state, launch, repair, and rollback.
- L5: the engine and command registry expose every user-visible action.
- L6: native egui UI and automation/MCP surfaces.
- `apps/photocraft` already contains macOS window, menu, monitor-profile, and Apple-event integration.
- Packaging exists for Windows, Linux, and FreeBSD; a qualified macOS application package/release path is not present.

## Portability inventory

Already portable or macOS-aware:

- Native Rust/egui/wgpu editor and most engine/document crates.
- ComfyUI HTTP/WebSocket client and built-in-node workflow generation.
- Existing external-ComfyUI discovery/connection boundary.
- macOS application shell components.
- Checksum, resumable download, archive traversal defense, staged install state, and owned-process concepts.

Windows/NVIDIA-bound:

- Host probing is centered on Windows system JSON, `nvidia-smi`, paging files, and Visual C++ runtime state.
- The managed manifest selects Windows portable ComfyUI CUDA archives (`nvidia`/`nvidia_cu126`).
- Runtime layout and launch assumptions include the Windows portable archive and embedded Python.
- GPU qualification, low-memory guidance, installer UX, issue templates, and release scripts assume Windows/NVIDIA.
- Packaging/signing/publishing automation is Windows-first for LayerShop.

Required Apple Silicon seams:

- A narrow macOS probe implementation for architecture, unified memory, storage, and OS version.
- A pinned ARM64 Python/ComfyUI/PyTorch-MPS runtime manifest independent of the Windows portable layout.
- macOS application-support/cache/log/model paths, loopback-only launch, ownership records, health checks, staged activation, and rollback.
- Memory-aware model/workflow qualification on 24 GB unified memory.
- ARM64 app packaging, hardened runtime, signing, notarization, DMG, and transactional updates behind explicit release approval.

## Reference host evidence

- Mac model `Mac16,10`, `arm64`, 10 physical CPU cores, 24 GiB unified memory.
- macOS 27.2 (build 26B123), APFS internal SSD, 175+ GiB available during qualification.
- Xcode 27.1 and SDK 27.0 selected; one valid Apple Development identity was found without exposing certificate material.
- Rust 1.95.0 was installed because the workspace declares `rust-version = "1.95"`.
- Unmodified baseline tests passed on this host: 100 `photocraft-aisetup` tests and 154 `photocraft-aigen` tests; four explicit network/hardware tests were skipped.
- Ollama 0.40.15 is loopback-local and cloud features are disabled.
- `qwen3.8:27b` loaded at 18 GB but failed its first Metal command with an out-of-memory error. It is not qualified for this 24 GB workflow.
- `qwen3.5:9b` uses about 7.6 GB on disk, peaked below 10 GiB in the OpenCode smoke run, completed a real read/tool call, reported zero provider cost, and unloaded before compilation.

## Protection and access status

The connected GitHub application has pull and push access to the private repository. The local Mac does not yet have a GitHub SSH key or HTTPS credential accepted by GitHub, so an unattended local push cannot be enabled. `SILICA_PUSH` remains off, and the launch agent must not be activated until the private histories are merged and a repository-scoped credential passes a non-destructive push test.

Branch protection, default workflow permissions, contribution rules, and secret-scanning settings must be verified in GitHub before implementation branches can merge. No automatic merge, signing, notarization, release, or public publication is authorized.
