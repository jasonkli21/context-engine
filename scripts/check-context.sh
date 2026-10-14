#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

required=(
  "AGENTS.md"
  "README.md"
  "docs/README.md"
  "docs/current-state.md"
  "docs/prd.md"
  "docs/architecture.md"
  "docs/plans/phase-0-implementation-plan.md"
  "docs/development/llm-workflow.md"
)

for path in "${required[@]}"; do
  if [[ ! -f "$path" ]]; then
    echo "context check failed: missing $path" >&2
    exit 1
  fi
done

agents_bytes=$(wc -c < AGENTS.md | tr -d ' ')
if (( agents_bytes > 5120 )); then
  echo "context check failed: AGENTS.md is ${agents_bytes} bytes; keep mandatory agent context <= 5120 bytes" >&2
  exit 1
fi

if [[ -f CLAUDE.md ]] && ! grep -q 'AGENTS.md' CLAUDE.md; then
  echo "context check failed: CLAUDE.md must point to AGENTS.md rather than define a parallel ruleset" >&2
  exit 1
fi

if [[ -f .github/copilot-instructions.md ]] && ! grep -q 'AGENTS.md' .github/copilot-instructions.md; then
  echo "context check failed: Copilot instructions must route to AGENTS.md rather than duplicate it" >&2
  exit 1
fi

for bad in handoff.md HANDOFF.md session-notes.md scratchpad.md; do
  if [[ -e "$bad" ]]; then
    echo "context check failed: root-level transient context file $bad should not become canonical" >&2
    exit 1
  fi
done

echo "context checks passed"
