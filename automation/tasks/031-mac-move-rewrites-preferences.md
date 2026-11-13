# The Mac data-folder move also points the stored preferences at the new folder

P1 bug from task 029. On macOS, `migrate_data_folder` in `apps/photocraft/src/app_dirs.rs` (about line 592) renames `~/Library/Application Support/LayerShop` to `…/SilicaLyr` and then rewrites the stored paths in `ai-setup.json` only. But `preferences.json` in the same folder stores the Local AI settings (`photocraft_engine::prefs`, field `ai: photocraft_aigen::Settings`, about line 1089 of `crates/engine/src/prefs.rs`), and its `comfy_launch` holds absolute paths into the engine folder (`python` and `folder`, set from `Layout` by `crates/aisetup/src/runtime.rs` `build_launch_command`, about line 291). After the move those still name `…/LayerShop/LocalAI/engine/…`, which no longer exists, so a user whose engine was installed before the rebrand finds Local AI won't start.

Scope (macOS-only code, `#[cfg(target_os = "macos")]` as now):

- In `migrate_data_folder`, after a successful rename, run the existing `rewrite_record_paths` on `preferences.json` as well as `ai-setup.json`, with the same old and new prefixes. Keep it best-effort and never fatal, exactly as for the record: a missing, unparsable or oversized file is left as it is.
- Rename `ASETUP_RECORD_MAX` to a name that fits both files (for example `REWRITE_MAX`) and update its doc comment: the cap is 1 MiB for the record; use the same cap for preferences. Update the doc comments of `migrate_data_folder` and `rewrite_record_paths` to say both files are rewritten.
- No other files change.

Tests, in `apps/photocraft/src/app_dirs/tests.rs` (macOS-only like the others there):

- Extend or add a test: an old folder whose `preferences.json` is JSON like `{"ai": {"comfy_launch": {"python": "<old>/LocalAI/engine/python/bin/python3", "folder": "<old>/LocalAI/engine", "args": "-s main.py"}}, "recentFiles": ["/Users/dev/Pictures/a.pcraft"], "unknownField": 7}` moves; afterwards `python` and `folder` start with the new folder, `args`, the outside path and the unknown field are unchanged, and no `preferences.json.new` is left.
- The existing test that writes `preferences.json` as the non-JSON bytes `settings` must still pass unchanged (an unparsable file is left as it is).

Edit only `apps/photocraft/src/app_dirs.rs` and `apps/photocraft/src/app_dirs/tests.rs`. Do not add files. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes, including the new test on the Mac host.
