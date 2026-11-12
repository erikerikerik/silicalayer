# macOS ARM64 Managed Runtime Artifact Qualification

This records the first complete local proof of a relocatable managed runtime for the reference Apple Silicon host. It extends the model-fit result in [MPS_MODEL_SPIKE.md](MPS_MODEL_SPIKE.md): the Python/ComfyUI/PyTorch environment was assembled as an archive, extracted through PhotoCraft's production hostile-archive boundary, launched from the extracted location, and used for a real generation.

## Result

**Pass for local artifact construction, secure extraction, MPS startup, and generation.** The artifact is not published and is not in the bundled setup manifest. No release, public upload, paid API, cloud inference, or repository asset was created.

The local evidence is under the ignored `.silica-agent/runtime-build/` folder. It may be deleted and is not an installation source.

## Pinned inputs

| Component | Pin |
|---|---|
| Python | CPython 3.12.14, Astral python-build-standalone release `20260901`, Apple Silicon `install_only` |
| Python input SHA-256 | `3ee3ee547cedfeb7c2b16b2b7156039f7b470bb8f857e226fd3d2eb11db83c76` |
| ComfyUI | 0.43.0, commit `426f5eea52a7ece1dd599fff4cb6cd1f352ddd24` |
| PyTorch | 2.12.1 |
| torchvision | 0.27.1 |
| torchaudio | 2.11.0 |
| Architecture | `arm64` |

The standalone Python distribution was selected because its upstream describes `install_only` builds as self-contained and portable to any path. The three PyTorch pins are the official ComfyUI standalone Mac MPS pins and match the environment used in the preceding model spike.

## Artifact

| Property | Observed value |
|---|---:|
| Archive format | 7z, LZMA2 |
| Archive bytes | 760,253,266 |
| Archive SHA-256 | `891e8e2af644353deeba13ff122b3b4c23589f7b6611c334334f75c2f0b0a32f` |
| Unpacked bytes declared by the archive | 1,887,798,757 |
| Files | 64,653 |
| Longest relative path | 173 characters |
| Symbolic links | 0 |
| Python path | `python/bin/python3` |
| ComfyUI script | `ComfyUI/main.py` |
| Executable allowlist | `python/bin/python3` |

Models are deliberately not in the runtime archive. They remain checksum-verified, separately resumable files in the managed models folder, matching the existing setup architecture.

## Extraction and launch proof

The archive passed `photocraft-aisetup`'s production header inspection and `unpack` implementation with default limits. Extraction restored mode `0755` only on the manifest-declared Python executable; `ComfyUI/main.py` remained a non-executable regular file. The extracted tree still contained no symbolic links.

From the extracted location:

- Python reported its new extracted prefix rather than the build prefix.
- `pip check` reported no broken requirements before packaging.
- PyTorch reported MPS built and available.
- ComfyUI reported Python 3.12.14, ComfyUI 0.43.0, device `mps`, and shared memory.
- `/system_stats` and `/object_info` both answered successfully on loopback.
- The server stopped cleanly after the test.

## Generation proof

The extracted runtime ran the same PhotoCraft Z-Image Turbo workflow used for the original MPS spike, reading the separately stored checksum-verified model files through ComfyUI's extra-model-paths boundary.

| Property | Observed value |
|---|---:|
| Output | 1008 x 1008 PNG |
| Wall time from submission to retrieval | 360.19 s |
| ComfyUI prompt time | 358.87 s |
| Diffusion | 8 steps, about 41 s per step |
| Output bytes | 1,751,892 |
| Output SHA-256 | `72377213b643d5a32b1eba34d190cf92e0b3da07feb99ea802c265083859c6e9` |

ComfyUI loaded the 7,672.25 MB text encoder, detected the INT8 ConvRot quantization metadata, loaded 5,889.08 MB of diffusion weights, partially offloaded them for VAE decode, and completed without an out-of-memory error or crash.

## What remains before enabling installation

1. Produce a reviewed lock with hashes for every Python wheel. ComfyUI pins its own packages and the Mac PyTorch trio, but its full requirements file leaves several transitive versions open.
2. Inventory and approve all redistribution licences and notices. The archive includes GPL-3.0 ComfyUI and a standalone Python distribution with multiple licence texts; these are outside this repository's normal dependency allowlist and require an explicit maintainer decision before commit or distribution.
3. Choose an authenticated or public artifact host. The current manifest accepts HTTPS GitHub release assets, while a private GitHub release cannot be downloaded by the existing unauthenticated fetcher. Publishing the artifact is intentionally not authorized by this qualification.
4. Turn the local assembly into a deterministic, reviewed build recipe and reproduce the archive checksum on a clean builder.
5. Sign and notarize the nested executable content as part of the later application packaging work.
6. Only after those gates pass, add `macos_arm64` to the bundled manifest and enable the Apple Silicon setup path.

