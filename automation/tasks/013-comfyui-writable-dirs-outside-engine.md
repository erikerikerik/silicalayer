# Keep ComfyUI's writable folders outside the managed engine

Recoverability and security fix. The managed engine folder is verified, marked and replaced as a unit on update, repair and rollback, but ComfyUI by default writes `user/` (its database and settings), `output/`, `input/`, `temp/` and loads code from `custom_nodes/` inside its own folder. An engine swap therefore loses or mixes user data, and anything placed in `custom_nodes/` runs at the next start.

Scope (macOS managed runtime only; Windows launch stays identical):

- In `crates/aisetup/src/plan.rs` `Layout`, add a per-install data folder beside, not inside, the engine version folder (for example `<engine_dir>/data/`), with `user`, `input`, `output` and `temp` subfolders. Creating them is part of the existing configure step and must be idempotent.
- In `build_launch_command` for `RuntimePlatform::MacosArm64` only, add `--user-directory`, `--input-directory`, `--output-directory` and `--temp-directory` pointing at those folders, plus `--listen 127.0.0.1` and `--disable-api-nodes`. Keep every flag in one documented constant list so it can be checked against ComfyUI 0.43.0's `comfy/cli_args.py` during the wizard test.
- Set these environment variables on the managed macOS launch only: `HF_HUB_OFFLINE=1`, `TRANSFORMERS_OFFLINE=1`, `HF_HUB_DISABLE_TELEMETRY=1`, `DO_NOT_TRACK=1`. If `Launch` has no environment field, add one without changing Windows output.
- Removal keeps the data folder unless the user chose to remove everything, matching how models are preserved today.
- Do not make the engine read-only in this task, and do not change downloads, manifests, archives or Windows behavior.

Tests (each must fail without the fix): the macOS launch command contains each flag once with paths outside `engine_root` and survives paths with spaces through `join_args`/`split_args`; the Windows command is byte-for-byte unchanged; the data folders are outside `engine_root`; removing the engine leaves the data folder.

Exit evidence: the full supervisor gate passes.
