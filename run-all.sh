#!/usr/bin/env bash
# run-all.sh [--only mech|agent] [--models sonnet,opus] [--real <repos.tsv>]
# The whole lab: build the binary under test, the mechanical harness, the
# agent matrix, the scores. See ci/agent-lab/README.md.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ONLY=""; MODELS="sonnet,opus"; REAL=""
while [ $# -gt 0 ]; do
  case "$1" in
    --only) ONLY=$2; shift 2 ;;
    --models) MODELS=$2; shift 2 ;;
    --real) REAL=$2; shift 2 ;;
    *) echo "unknown: $1" >&2; exit 2 ;;
  esac
done
( cd "$HERE" && cargo build --release -q )
export PATH="$HERE/target/release:$PATH"
if [ "$ONLY" != agent ]; then
  "$HERE/ci/agent-lab/mech/run.sh"
fi
if [ "$ONLY" != mech ]; then
  "$HERE/ci/agent-lab/agent/run-matrix.sh" --models "$MODELS"
  if [ -n "$REAL" ]; then "$HERE/ci/agent-lab/agent/run-real.sh" "$REAL" --models "$MODELS"; fi
  "$HERE/ci/agent-lab/agent/score.sh"
fi
