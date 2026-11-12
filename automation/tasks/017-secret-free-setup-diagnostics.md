# A secret-free diagnostics report for local AI setup

V1 requires diagnostics that contain no secrets (`docs/SILICALAYER_ROADMAP.md`, V1 items 10 and 13). Today a user with a failed setup has nothing to send. Add a read-only report; no network, no uploading, no UI beyond what is listed.

Scope:

- New module `crates/aisetup/src/diagnostics.rs` (declare it in `crates/aisetup/src/lib.rs` next to the other `pub mod` lines). It holds:
  - `pub fn redact(text: &str) -> String`: replaces Hugging Face tokens (`hf_` followed by 20+ letters/digits), `Bearer <token>`, `Authorization:` header values, `sk-…` and `gh[pousr]_…` keys, URL userinfo (`https://user:pass@host`) and query values for keys named `token`, `key`, `access_token` or `signature` with `[redacted]`. The user's home folder in paths becomes `~`.
  - `pub fn report(...) -> serde_json::Value` taking already-gathered facts as parameters (pure, testable): app version, OS name/version/architecture, unified memory or card memory, runtime target and qualified flag, the setup record's choices with every credential field removed (keep only booleans such as whether a token exists), the engine marker (platform, version, variant), the last finished result or error text, free space for the engine and models folders, and the last 200 lines of the engine log. Every string goes through `redact`.
- `crates/engine/src/ai_setup_cmds.rs`: a new command `ai.setup.diagnostics` in `specs()` (copy the shape of the `ai.setup.status` spec; `journal: false`; read-only, changes nothing) that gathers those facts from the session, the setup record, the marker and the engine log file, and returns `report(...)`. The engine log path is the launch log used by `crates/aigen/src/launch.rs` (`fn log_file`, about line 78); if it cannot be found, report `null` for the log, never an error.
- `crates/ui-egui/src/ai_setup_ui.rs`: one "Copy diagnostics" button on the setup wizard's failure or status view that runs the command and copies the pretty JSON to the clipboard. Nothing is written to disk or sent anywhere.

Tests (each must fail without the change), in `crates/aisetup/src/diagnostics.rs`:

- `redact` removes each secret form above, including one embedded mid-line in a log, and leaves ordinary text, version numbers and SHA-256 hashes (64 hex characters) unchanged.
- `report` with a record that holds a token outputs no token value anywhere (serialize the report and search for the token string), and still says a token exists.
- The log tail is limited to 200 lines and redacted.
- An engine test (name containing `ai_setup`) that `ai.setup.diagnostics` succeeds with no setup at all.

Edit only these files and their existing test modules. Do not read `target/` or anything outside the repository.

Exit evidence: the full supervisor gate passes.
