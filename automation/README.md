# Local autonomous development

This directory runs one bounded SilicaLayer task at a time on the reference Apple Silicon Mac. It uses OpenCode only with the loopback Ollama provider. Paid and remote providers, web access, subagents, external-directory access, and interactive questions are denied in `opencode.json`.

The worker deliberately separates inference from compilation. Supervisor policy, task definitions, and CI are protected from model edits:

1. OpenCode reads and edits with Ollama loaded.
2. The worker unloads the model.
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

The launch agent runs every 30 minutes and after login. Install it only after `preflight` passes. Uninstall with `sh automation/uninstall-launch-agent`.

## Environment overrides

- `SILICA_MODEL` (default `qwen3.5:9b`; the 27B model exhausted Metal memory on the 24 GB reference Mac)
- `SILICA_MAX_REPAIRS` (default `3`, maximum `5`)
- `SILICA_MAX_FAILED_RUNS` (default `1`, range `1`–`3`; exhausted runs quarantine the task)
- `SILICA_AGENT_TIMEOUT` and `SILICA_GATE_TIMEOUT` (default `3600` seconds, maximum `7200`)
- `SILICA_PUSH` (`0` by default)
- `SILICA_GATE` (default `automation/gate`)
- `SILICA_MIN_FREE_GB` (default `80`)

No setting enables release publication, signing, notarization, secret access, destructive Git, or paid APIs. The worker accepts only `automation/*` or `silicalayer/*` branches and an exact private SilicaLayer origin URL.
