# MPS Model Spike

This records the first hardware qualification of the intended local generation path on the reference Mac. It answers the gating question: a useful one-megapixel model fits and completes through PhotoCraft's existing ComfyUI integration on a 24 GB Apple Silicon host.

## Result

**Pass for the default text-to-image path.** Z-Image Turbo generated a valid 1008×1008 PNG on the M4 Mac mini in 366.28 seconds end to end. ComfyUI reported 364.99 seconds for the prompt and 334 seconds for the eight diffusion steps. No out-of-memory error, process crash, silent corruption, custom node, cloud service, or paid API was involved.

The result qualifies ComfyUI/PyTorch-MPS as the primary V1 engine direction. MLX is not needed as a contingency for this path. It does not yet qualify repeated runs, cancellation, editing, inpainting, thermals, or managed installation and recovery; those remain Phase 4 work.

## Reference host

- Mac mini with Apple M4, 10 CPU cores, and 24 GB unified memory
- macOS 27.2 (26B123)
- Native ARM64 Homebrew Python 3.12.16 in an isolated virtual environment
- PyTorch 2.12.1, torchvision 0.27.1, torchaudio 2.11.0
- ComfyUI 0.43.0 at commit `426f5eea52a7ece1dd599fff4cb6cd1f352ddd24`
- MPS built and available; ComfyUI selected device `mps` with shared memory

The runtime listened on `127.0.0.1` only and loaded no custom nodes.

## Qualified files

The three files are already declared by `crates/aisetup/manifest.json`. Every downloaded file passed its declared SHA-256 check.

| Role | File | Size | SHA-256 |
|---|---|---:|---|
| Diffusion model | `z_image_turbo_int8_convrot.safetensors` | 6,201,001,296 bytes | `be517ebd47c912a5626a588e1aeea43e6be4a43c0cdcd2b48a2a780d9f358635` |
| Text encoder | `qwen_3_4b.safetensors` | 8,044,982,048 bytes | `6c671498573ac2f7a5501502ccce8d2b08ea6ca2f661c458e708f36b36edfc5a` |
| VAE | `ae.safetensors` | 335,304,388 bytes | `afc8e28272cd15db3919bacdb6918ce9c1ed22e96cb12c4d5ed0fba823529e38` |

Total declared download size is 14,581,287,732 bytes (13.58 GiB).

## Measured run

The existing ignored integration test was used so the experiment exercised the application's real workflow builder, HTTP transport, progress handling, history polling, and image download:

```sh
PHOTOCRAFT_COMFY_URL=http://127.0.0.1:8199 \
  cargo test -p photocraft-aigen --lib a_real_render_says_each_step \
  -- --ignored --nocapture
```

Prompt: `a lighthouse on a cliff at dusk`

| Observation | Measurement |
|---|---:|
| Output | valid RGB PNG, 1008×1008, 1,688,670 bytes |
| End-to-end time | 366.28 s |
| ComfyUI prompt time | 364.99 s |
| Eight sampling steps | 334 s total, 41.76 s/step average |
| Text encoder loaded | 7,672.25 MB reported by ComfyUI |
| Diffusion model loaded | 5,889.08 MB reported by ComfyUI |
| VAE loaded after partial model offload | 159.87 MB reported by ComfyUI |
| Lowest sampled system memory free | 21% |
| Swap before / highest sample / after | 9.33 / 10.06 / 9.79 GiB |

Swap was already heavily used before the run, so its absolute value is not attributable to inference. The observed increase was about 0.73 GiB, the run completed, and the system recovered to 70% free memory after ComfyUI offloaded the active models. A clean-boot repeated-run test is still required for a reliable peak-memory number.

The saved local evidence is intentionally outside Git under `.silica-agent/mps-spike/`, including the generated PNG, logs, ComfyUI checkout, virtual environment, and model weights.

## Test-harness detail

`Family::ZImage.preset()` names the BF16 diffusion file, while the managed manifest intentionally installs the smaller INT8 ConvRot file and runtime discovery normally replaces the preset filename. The isolated test did not run setup discovery, so a local ignored symlink supplied the preset name while ComfyUI loaded the checksum-verified INT8 file. The server log confirmed the real source path was `z_image_turbo_int8_convrot.safetensors` and detected its quantization metadata. No repository file or weight was altered.

## Decision and next checks

Use Z-Image Turbo INT8 as the first 24 GB default candidate and continue the ComfyUI/MPS implementation. Keep generation sequential at batch size one and preserve staged loading/offload. The next runtime milestone is now recorded in [MACOS_RUNTIME_ARTIFACT.md](MACOS_RUNTIME_ARTIFACT.md): a 760 MB relocatable archive passed the production extractor and generated a fresh 1008×1008 image from its extracted location on this host.

Before Phase 4 exits, add:

1. two or more consecutive one-megapixel renders after a clean boot;
2. cancellation during text encoding, model loading, and sampling;
3. editing/inpainting and mask tests using models that support them;
4. thermal, peak-memory, sleep/wake, crash-recovery, and disk-reserve measurements;
5. an installed-runtime test without the test-only filename alias.
