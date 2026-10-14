#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

./scripts/check-context.sh

if ! command -v cargo >/dev/null 2>&1; then
  echo "verification failed: cargo is not installed" >&2
  exit 1
fi

cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features

echo "verification passed"
