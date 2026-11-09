# SilicaLayer

SilicaLayer is an Apple Silicon port of [LayerShop](https://github.com/SlavomirDurej/layershop), focused on preserving LayerShop's existing editor, layered AI workflow, and private managed ComfyUI experience.

The project exists to give professional graphic designers a practical, local-first AI tool that produces editable, intentional work rather than disposable “AI slop.”

## Current status

The LayerShop source history is now imported at baseline commit
`5d3cff0b210ff41999e5a0dc5747683d52383ea1`. Phase 0 and the initial M4 host
qualification are complete. Porting changes have not started yet; the repository
currently adds planning, audit evidence, and the bounded local development
supervisor.

The first implementation target is a Mac mini with:

- Apple M4
- 24 GB unified memory
- Current macOS
- 280 GB free internal storage
- 10 Gb Ethernet

V1 must run reliably on this 24 GB machine. Higher-memory Apple Silicon systems, including 36 GB configurations, should benefit without becoming the minimum requirement.

## Governing constraint

This is a port, not a ground-up rewrite.

We will preserve LayerShop's UI, editor architecture, layer workflow, ComfyUI client, workflow generation, model management, and cloud-provider boundary. We will replace or adapt only the Windows/NVIDIA-specific runtime, setup, packaging, and hardware-integration pieces that cannot work on Apple Silicon.

See:

- [Project brief](docs/PROJECT_BRIEF.md)
- [Engineering and security standards](docs/ENGINEERING_STANDARDS.md)
- [SilicaLayer roadmap and V1 definition](docs/SILICALAYER_ROADMAP.md)
- [Upstream LayerShop roadmap](docs/roadmap.md)
- [Machine handoff notes](docs/MACHINE_HANDOFF.md)

## Primary references

- [LayerShop repository](https://github.com/SlavomirDurej/layershop)
- [LayerShop ComfyUI guide](https://layershop.aardvarkstudios.net/comfyui/)
- [ArtCraft Suite](https://github.com/erikerikerik/artcraft-suite/) — reference for trust, installation, update, rollback, and AI-assisted development discipline

## Important boundaries

- Local generation is the default.
- Cloud generation is optional and uses user-provided credentials.
- The app manages its own private ComfyUI runtime.
- Built-in ComfyUI nodes are preferred; custom-node dependency sprawl is out of scope.
- MFLUX/MLX is a fallback integration only if an essential workflow cannot run reliably through ComfyUI on MPS.
- Product renaming and visual identity are separate from the port and must not trigger an unnecessary UI rewrite.
- No credentials, signing keys, API tokens, downloaded model weights, or machine-specific secrets belong in this repository.

## License and attribution

LayerShop is an upstream project owned by its original maintainers. Before distributing SilicaLayer, the upstream license, third-party licenses, model licenses, trademarks, and attribution requirements must be reviewed and honored.
