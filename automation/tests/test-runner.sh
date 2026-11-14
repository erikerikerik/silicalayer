#!/bin/sh
set -eu
ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
for file in automation/silica-agent automation/gate automation/install-launch-agent automation/uninstall-launch-agent; do
  sh -n "$ROOT/$file"
done
plutil -lint "$ROOT/automation/net.silicalayer.local-agent.plist.in" >/dev/null
grep -F '/.silica-agent/' "$ROOT/.gitignore" >/dev/null
grep -F '"resource": "ollama"' "$ROOT/opencode.json" >/dev/null
grep -F '"resource": "strata"' "$ROOT/opencode.json" >/dev/null
grep -F '"resource": "*", "effect": "deny"' "$ROOT/opencode.json" >/dev/null
if grep -E '"action": "shell".+"effect": "allow"' "$ROOT/opencode.json" >/dev/null; then
  printf 'OpenCode shell access must remain fully denied\n' >&2
  exit 1
fi
# Literal source checks; the variables must not expand in this test process.
# shellcheck disable=SC2016
grep -F 'QUARANTINED="$STATE/quarantined"' "$ROOT/automation/silica-agent" >/dev/null
# shellcheck disable=SC2016
grep -F 'task_failure "gate failed after $MAX_REPAIRS repairs' "$ROOT/automation/silica-agent" >/dev/null
grep -F 'format_changed "$RUN_LOG/format-$attempt.log"' "$ROOT/automation/silica-agent" >/dev/null
grep -F 'status) status;;' "$ROOT/automation/silica-agent" >/dev/null
grep -F 'cargo build -p photocraft' "$ROOT/automation/gate" >/dev/null
printf 'runner checks passed\n'
