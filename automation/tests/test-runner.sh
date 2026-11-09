#!/bin/sh
set -eu
ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
for file in automation/silica-agent automation/gate automation/install-launch-agent automation/uninstall-launch-agent; do
  sh -n "$ROOT/$file"
done
plutil -lint "$ROOT/automation/net.silicalayer.local-agent.plist.in" >/dev/null
grep -F '/.silica-agent/' "$ROOT/.gitignore" >/dev/null
grep -F '"resource": "ollama"' "$ROOT/opencode.json" >/dev/null
grep -F '"resource": "*", "effect": "deny"' "$ROOT/opencode.json" >/dev/null
printf 'runner checks passed\n'
