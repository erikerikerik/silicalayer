# Qualify Apple Silicon hosts by unified memory

Correctness fix. `Probe::runtime_target()` in `crates/aisetup/src/probe.rs` marks every 64-bit arm64 Mac as qualified, including 8 GB and 16 GB machines, and `vramGb` is null on Macs, so the memory-based preset logic (`LIGHT_BELOW_VRAM_GB` and the presets in `crates/aisetup/manifest.json`) never runs there. The only qualified configuration is Z-Image Turbo INT8 on 24 GB unified memory (`docs/MPS_MODEL_SPIKE.md`), and that run already reached 21% free memory.

Scope:

- Add one named constant for the minimum qualified unified memory: 24 GiB.
- `runtime_target()` on Apple Silicon: qualified only when `os.unified_memory_mb` is known and at least that minimum. Unknown memory is not qualified.
- `can_install()` keeps allowing an unqualified Apple Silicon Mac only behind the same explicit "install anyway" path Windows uses for unqualified variants; do not silently block or silently allow.
- The probe's `gpu` check row: below the minimum, a `Warn` explaining that local models may not fit and cloud models work; at or above, `Ok` as today.
- Report a memory figure for Macs wherever the setup command reports `vramGb`, derived from unified memory, so preset selection and the light-preset suggestion work on Macs. Keep the Windows value unchanged.
- No new dependencies, no change to downloads, manifests' files, or Windows behavior.

Tests (each must fail without the fix): 8 GB, 16 GB, 23.9 GB, unknown, 24 GB and 64 GB Apple Silicon probes give the expected qualified flag, check level and memory figure; a Windows probe is unchanged.

Exit evidence: the full supervisor gate passes.
