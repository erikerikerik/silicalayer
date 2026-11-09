# Project Brief

_Last updated: November 9, 2026_

## Purpose

SilicaLayer is intended to give professional graphic designers a serious local-first AI-assisted image editor. The audience may have no Python, Terminal, Git, or ComfyUI experience. Setup must therefore be guided, safe, recoverable, and largely automatic.

The product should help trained designers combine generative tools with layers, masks, composition, retouching, and deliberate art direction. It is not intended to maximize raw image volume or imitate one-click prompt sites.

## Source and relationship to LayerShop

SilicaLayer begins with [LayerShop](https://github.com/SlavomirDurej/layershop). The upstream repository is not owned by this project.

The baseline product behavior is the experience described in LayerShop's [ComfyUI guide](https://layershop.aardvarkstudios.net/comfyui/):

1. The editor constructs a ComfyUI workflow behind the scenes.
2. The user selects creative settings instead of manually editing a node graph.
3. Generation results return to the document as layers.
4. A guided setup installs and manages a private ComfyUI/Python environment.
5. Model discovery and workflow configuration remain inside the application.
6. An explicitly configured external ComfyUI server may be supported without taking ownership of its lifecycle.

SilicaLayer should remain structurally recognizable as a LayerShop port. Upstream-friendly changes are preferred where practical.

## Product decisions already made

### Preserve

- Existing editor and UI structure
- Layer-based generation and editing
- ComfyUI HTTP/WebSocket integration
- Existing workflow generation and model definitions where compatible
- Setup-wizard concepts and progress reporting
- Model manager
- Cloud-provider boundary
- Generated-layer metadata, masks, and document behavior
- Built-in ComfyUI nodes as the default compatibility target
- Existing macOS application and packaging work where usable

### Adapt for Apple Silicon

- macOS and Apple Silicon hardware probing
- Unified-memory and free-storage checks
- Detection of a user-configured ComfyUI server
- Managed ARM64 Python environment
- Pinned ComfyUI and PyTorch/MPS runtime
- Runtime manifest and downloads
- Launch arguments, environment variables, and process supervision
- macOS application-support, cache, log, and model paths
- Secure archive extraction and executable permissions
- Memory-aware model catalog and sampler defaults
- API credential storage in macOS Keychain
- Signing, notarization, DMG packaging, and updates
- Product name, icon, bundle identifier, About screen, and installer identity

### Do not rebuild without necessity

- The editor
- The layer/document model
- The AI command interface
- Workflow authoring that already functions with built-in ComfyUI nodes
- Cloud support that can be retained behind the existing provider interface
- The visual UI merely to create a new brand

## Managed engine decision

The default is a private managed engine, not a requirement that users install or administer ComfyUI Desktop.

On first run the application should:

1. Inspect the Mac and available storage.
2. Detect an explicitly configured external ComfyUI server if the user chooses that route.
3. Otherwise install a private, pinned ARM64 Python/ComfyUI environment.
4. Validate every downloaded artifact.
5. Install or select a model appropriate for the machine.
6. Start the private server on loopback only.
7. verify readiness before enabling generation.
8. Stop only processes owned by SilicaLayer.

The managed engine must be replaceable and updateable without coupling it tightly to editor code.

## Local and cloud operation

Local inference is the primary experience. Cloud generation is optional for workflows or models that exceed local hardware, including services offering models such as FLUX.3.

Cloud requirements:

- Bring-your-own credentials
- Credentials stored in Keychain, never configuration files or logs
- Clear provider and cost disclosure before use
- No silent fallback from local to paid cloud generation
- Provider integrations behind the existing engine/provider boundary
- Local documents and assets uploaded only after an explicit user action

## Model strategy

V1 must include one recommended image-generation/editing configuration that fits a 24 GB unified-memory Mac.

Selection criteria:

- Reliable ComfyUI/MPS operation
- Useful output for professional design work
- Inpainting or editing support where possible
- Quantization or staged loading when beneficial
- Clear license display, especially commercial-use terms
- Reproducible pinned downloads and checksums
- Reasonable generation time and thermal behavior on the reference Mac

A 36 GB system may support larger configurations, but V1 cannot require it.

### LoRA and SSD conclusions

LoRAs are relatively small low-rank adapters; they do not replace the base model. The base model must still be available in memory during inference.

SSD-backed expert streaming is effective for sparse mixture-of-experts LLMs because only a subset of experts is active per token. Dense diffusion models repeatedly access much more of their weights across denoising steps, so using SSD as pseudo-RAM is a slow fallback rather than the primary design.

Prefer:

1. A model that genuinely fits 24 GB
2. Quantization
3. Staged component loading and unloading
4. Memory-aware resolution and batch limits
5. Cloud execution for genuinely oversized jobs

### MLX/MFLUX boundary

MFLUX or another MLX-native engine is a contingency, not the first implementation plan. It should be considered only when an essential workflow cannot run acceptably through ComfyUI/MPS. If used, it must sit behind the existing provider/engine interface so the editor remains unchanged.

## Reference hardware and storage policy

### Development and minimum qualification machine

- Mac mini
- Apple M4
- 24 GB unified memory
- Current macOS
- 280 GB free internal storage at project start
- 10 Gb Ethernet
- Paid Apple Developer membership available

### Storage allocation

Keep approximately 70–80 GB of internal storage free for macOS, swap, updates, and operational safety. The remaining space may hold:

- Xcode and developer tools
- Source and build outputs
- Managed ComfyUI runtime
- Active model set
- Test documents and release staging

A Crucial X10 Pro 2 TB external SSD may be used for archived builds, additional models, caches, backups, and noncritical artifacts. Internal storage remains preferred for macOS, Xcode, Keychain, swap, the active checkout, final clean-install testing, and initially the active model.

## Branding

“SilicaLayer” is the current project/repository identity. Branding work is intentionally shallow until the port is functional:

- Application name
- Icon and logo
- Bundle identifier
- About screen
- DMG and installer presentation
- Documentation identity

Branding does not authorize redesigning the editor UI.

## Distribution intent

If the result is useful and legally distributable, the work should be published for others. Distribution requires:

- Upstream license and attribution review
- Third-party dependency license inventory
- Model license and commercial-use disclosure
- Signed and notarized macOS releases
- Reproducible release provenance
- No bundled secrets or private credentials
