# Local autonomous development

This directory runs one bounded SilicaLayer task at a time on the reference Apple Silicon Mac. It uses OpenCode only with a self-hosted Ollama model or a private Strata model reached through Tailscale. Paid providers, web access, subagents, external-directory access, interactive questions, and model shell commands are denied in `opencode.json`.

The worker deliberately separates inference from compilation. Supervisor policy, task definitions, and CI are protected from model edits:

1. OpenCode reads and edits with the configured self-hosted model.
2. The worker unloads a Mac-local Ollama model; a remote Strata model consumes no build memory on the Mac.
3. Cargo formats, checks, and tests with the memory available.
4. A fresh OpenCode session repairs a failure, up to the retry limit.
5. A fresh review session inspects the diff; the gates run again.
6. A verified change is committed. Push is opt-in and never targets `main`.

Runtime logs, locks, checkpoints, completion markers, and quarantine records live in `.silica-agent/`, which is ignored by Git. A killed run resumes from the dirty tree and test phase rather than asking the model to implement the task again. A task that exhausts its repair budget is quarantined and skipped on later launchd runs instead of looping forever.

## Commands

```sh
sh automation/silica-agent preflight
sh automation/silica-agent once
sh automation/silica-agent status
sh automation/install-launch-agent
launchctl kickstart -k gui/$(id -u)/net.silicalayer.local-agent
```

`once` uses the first task in `automation/tasks/` without a matching `.silica-agent/completed/<task>` or `.silica-agent/quarantined/<task>` marker. Quarantined tasks require human review; remove their marker only after resolving the recorded cause. Set `SILICA_PUSH=1` only after a repository-scoped Git credential has been installed and tested. The worker refuses `main` and `master`, refuses a dirty tree containing paths outside the active task, never force-pushes, never merges, and never creates a release.

The launch agent checks for work every 5 minutes and after login; the worker lock keeps one task running at a time, so the next queued task starts within 5 minutes of the last one finishing. Install it only after `preflight` passes. Uninstall with `sh automation/uninstall-launch-agent`.

## Environment overrides

- `SILICA_MODEL` (defaults to `qwen3.5:9b` for Ollama and the qualified UD-IQ4_XS model for Strata; the local 27B Ollama model exhausted Metal memory on the 24 GB reference Mac)
- `SILICA_PROVIDER` (`ollama` by default; `strata` selects the private remote backend)
- `SILICA_STRATA_BASE_URL` (required for Strata; an explicit Tailscale IPv4 URL on port 8080)
- `SILICA_MAX_REPAIRS` (default `3`, maximum `5`)
- `SILICA_MAX_FAILED_RUNS` (default `1`, range `1`–`3`; exhausted runs quarantine the task)
- `SILICA_AGENT_TIMEOUT` and `SILICA_GATE_TIMEOUT` (default `3600` seconds, maximum `7200`)
- `SILICA_PUSH` (`0` by default)
- `SILICA_GATE` (default `automation/gate`)
- `SILICA_MIN_FREE_GB` (default `20`)

No setting enables release publication, signing, notarization, secret access, destructive Git, or paid APIs. The worker accepts only `automation/*` or `silicalayer/*` branches and an exact private SilicaLayer origin URL.

Machine-specific backend values may be kept in ignored `.silica-agent/backend.conf` as `SILICA_PROVIDER`, `SILICA_MODEL`, and `SILICA_STRATA_BASE_URL`. The file is parsed as data, never sourced as shell code, and OpenCode is denied access to `.silica-agent/**`.
