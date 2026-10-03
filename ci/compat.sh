#!/usr/bin/env bash
# ci/compat.sh <old-binary> <new-binary>
#
# One machine, one new docsys, two repositories that have not moved on the
# same day (D-118). Repository A stays at docsys/0.4; repository B is upgraded
# to docsys/0.5. Both start from what the old docsys adopted.
#
#   1. A: the new docsys reports exactly the old one's findings, apart from
#      the one notice line naming `docsys upgrade`.
#   2. A: the new docsys refreshes a pin and records a verification; the old
#      docsys then runs lint, refs and the range gate on it — green, nothing
#      written that it does not read.
#   3. A and B: the relays, the git gate and a pin refresh work under the new
#      docsys; A's verification too, while B's pages carry none and `verify`
#      is refused there (D-130); A's CI commands pass with the old docsys,
#      B's with the new one.
#   4. B under the old docsys: every relay and the git gate say, in exactly one
#      line, which docsys the tree needs, and stop. (A gate in warn mode prints
#      the line and lets the commit go on; the repositories here adopt clean,
#      so theirs is hard.)
#   5. An assistant's base made inside an adopted project leaves the project's
#      git gate as the old docsys leaves it, on a 0.4 and a 0.5 project alike,
#      and the project's next commit lands.
#
# Manual, not CI: it needs the previous release's binary. It writes only a
# temporary directory.
set -euo pipefail

if [ $# -ne 2 ]; then
  echo "usage: $0 <old-binary> <new-binary>" >&2
  exit 2
fi
abs() { echo "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"; }
OLD=$(abs "$1")
NEW=$(abs "$2")
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/old-bin" "$WORK/new-bin"
ln -s "$OLD" "$WORK/old-bin/docsys"
ln -s "$NEW" "$WORK/new-bin/docsys"
OLD_PATH="$WORK/old-bin:$PATH"
NEW_PATH="$WORK/new-bin:$PATH"

say()  { printf '\n== %s\n' "$*"; }
fail() { printf 'COMPAT FAIL: %s\n' "$*" >&2; exit 1; }
g()    { git -c commit.gpgsign=false "$@"; }
with_old() { PATH="$OLD_PATH" "$@"; }
with_new() { PATH="$NEW_PATH" "$@"; }

printf 'old: %s\nnew: %s\n' "$("$OLD" --version 2>/dev/null || echo '(no --version)')" "$("$NEW" --version)"

# A repository as the old docsys adopts it: one reference page pinned to one
# function, committed through the old gate.
make_repo() {
  local dir=$1
  mkdir -p "$dir" && cd "$dir"
  g init -q -b main
  g config user.email compat@example.invalid
  g config user.name compat
  printf '# app\n' >README.md
  mkdir -p src
  cat >src/auth.rs <<'EOF'
pub fn refresh_token(ttl: u64) -> u64 {
    ttl / 2
}
EOF
  # adopted clean, so the old docsys writes its gate in hard mode
  with_old docsys adopt >/dev/null
  grep -q '^docsys_gate_exit=1$' .git/hooks/pre-commit || fail "$(basename "$dir"): the old adopt wrote a warn-mode gate"
  printf '/// doc: refresh\n%s\n' "$(cat src/auth.rs)" >src/auth.rs
  mkdir -p docs/reference
  cat >docs/reference/refresh.md <<EOF
---
id: refresh
type: reference
updated: $(date +%Y-%m-%d)
---
# Refresh

This page states how a token is refreshed; read it before changing the TTL.

A token is refreshed at half its TTL.
EOF
  printf '\n- [[reference/refresh|Refresh]] -- when a token is refreshed.\n' >>docs/index.md
  with_old docsys pin reference/refresh src/auth.rs --symbol refresh_token >/dev/null
  g add -A
  with_old g commit -qm "adopt, one pinned page"
}

# The code under the pin moves and the page is re-read and refreshed: what
# every repository does every week.
refresh_pin() {
  local bin_path=$1
  sed -i.bak 's|    ttl / 2|    ttl.saturating_div(2)|' src/auth.rs && rm -f src/auth.rs.bak
  PATH="$bin_path" docsys pin --refresh reference/refresh >/dev/null
  g add -A
  PATH="$bin_path" g commit -qm "refresh: saturating" || fail "$(basename "$PWD"): the gate refused a refreshed pin"
}

# What the CI workflow runs, with the given docsys.
ci_green() {
  local bin_path=$1 base=$2
  PATH="$bin_path" docsys lint --root docs --repo . >/dev/null || fail "$(basename "$PWD"): CI lint"
  PATH="$bin_path" docsys refs --repo . --root docs >/dev/null || fail "$(basename "$PWD"): CI refs"
  PATH="$bin_path" docsys gate --repo . --root docs --range "$base...HEAD" >/dev/null || fail "$(basename "$PWD"): CI gate"
}

# Every relay with an empty payload: its exit code and its stderr lines.
relays() {
  local bin_path=$1
  for hook in session-intent pre-commit-docs post-edit-updated stop-docs-reminder; do
    [ -f ".claude/hooks/$hook.sh" ] || continue
    local err code
    err=$(printf '{}' | PATH="$bin_path" CLAUDE_PROJECT_DIR="$PWD" bash ".claude/hooks/$hook.sh" 2>&1 >/dev/null) && code=0 || code=$?
    printf '%s\t%s\t%s\n' "$hook" "$code" "$(printf '%s' "$err" | grep -c . || true)"
    printf '%s\n' "$err" | sed 's/^/    /' | grep -v '^    $' || true
  done
}

# Under the new docsys every relay runs to its end: exit 0, no version line.
relays_run() {
  local name=$1 out=$2
  printf '%s\n' "$out"
  while IFS=$'\t' read -r hook code _; do
    case $hook in "    "*|"") continue ;; esac
    [ "$code" = 0 ] || fail "$name: $hook exited $code under the new docsys"
  done <<<"$out"
  if printf '%s\n' "$out" | grep -q "pins docsys"; then
    fail "$name: a relay named a version under the new docsys"
  fi
}

say "1 · A at docsys/0.4: the same findings from both binaries"
make_repo "$WORK/a"
# a finding on each side, so an empty comparison cannot pass
printf '\nSee [[reference/nowhere]].\n' >>docs/reference/refresh.md
old_out=$(with_old docsys lint --root docs --repo . 2>&1 || true)
new_out=$(with_new docsys lint --root docs --repo . 2>&1 || true)
notice=$(printf '%s\n' "$new_out" | grep -c '^docsys: this tree declares docsys/0.4' || true)
[ "$notice" = 1 ] || fail "the new docsys printed $notice notice lines on a 0.4 tree"
printf '%s\n' "$old_out" | grep -q 'reference/nowhere' || fail "the comparison has no finding to compare"
diff <(printf '%s\n' "$old_out") <(printf '%s\n' "$new_out" | grep -v '^docsys: this tree declares') \
  || fail "the findings differ on a 0.4 tree"
g checkout -q -- docs/reference/refresh.md
echo "identical, apart from one notice line"

say "2 · A: written by the new docsys, read by the old one"
base_a=$(g rev-parse HEAD)
refresh_pin "$NEW_PATH"
grep -q '^    hash: "sha256:' docs/reference/refresh.md || fail "A: the refresh did not write the pin's hash into the page (0.4 format)"
[ ! -e docs/.pins ] && [ ! -e docs/.verifies ] || fail "A: the refresh wrote acknowledgements on a 0.4 tree"
# a maintainer records the verification, as 0.15.1 did
with_new docsys verify reference/refresh --by compat --commit >/dev/null || fail "A: verify failed"
grep -q '^verified_by: compat' docs/reference/refresh.md || fail "A: verify wrote no 0.4 record"
ci_green "$OLD_PATH" "$base_a"
echo "the old docsys reads it: lint, refs, gate green"

say "3 · B upgraded; both repositories under the new docsys"
make_repo "$WORK/b"
base_b=$(g rev-parse HEAD)
with_new docsys upgrade --apply --commit >/dev/null || fail "B: the upgrade did not land"
grep -q '^spec: docsys/0.5' docs/.docmeta.yml || fail "B: still declares 0.4"
sed -i.bak 's|^type: reference$|type: reference\nsources: [src/auth.rs]|' docs/reference/refresh.md && rm -f docs/reference/refresh.md.bak
g add -A; with_new g commit -qm "refresh rests on the code"
refresh_pin "$NEW_PATH"
[ -d docs/.pins/refresh ] || fail "B: the refresh wrote no acknowledgement"
grep -q '^pins:' docs/reference/refresh.md || fail "B: the page names no pins:"
head_b=$(g rev-parse HEAD)
if with_new docsys verify reference/refresh --commit >/dev/null 2>&1; then
  fail "B: verify ran on a 0.5 tree"
fi
[ "$(g rev-parse HEAD)" = "$head_b" ] || fail "B: a refused verify made a commit"
! grep -q '^verifi' docs/reference/refresh.md || fail "B: a verification field in the page"
ci_green "$NEW_PATH" "$base_b"
relays_run "B" "$(relays "$NEW_PATH")"
cd "$WORK/a"
relays_run "A" "$(relays "$NEW_PATH")"
ci_green "$OLD_PATH" "$base_a"
echo "A: relays run, CI green with the old docsys · B: relays run, CI green with the new docsys"

say "4 · B under the old docsys: one line, and a stop"
cd "$WORK/b"
out=$(relays "$OLD_PATH")
printf '%s\n' "$out"
while IFS=$'\t' read -r hook code lines; do
  case $hook in "    "*|"") continue ;; esac
  [ "$code" = 1 ] || fail "B: $hook exited $code under the old docsys"
  [ "$lines" = 1 ] || fail "B: $hook printed $lines lines under the old docsys"
done <<<"$out"
printf '%s\n' "$out" | grep -q "this tree pins docsys " || fail "B: the line does not name the version"
printf '\nmore\n' >>README.md
g add README.md
if commit_err=$(with_old g commit -qm "under the old docsys" 2>&1); then
  fail "B: the git gate let the old docsys commit"
fi
printf '%s\n' "$commit_err" | grep -q "this tree pins docsys " || fail "B: the gate did not name the version: $commit_err"
[ "$(printf '%s\n' "$commit_err" | grep -c .)" = 1 ] || fail "B: the gate printed more than its one line: $commit_err"
echo "every relay and the gate: one line, exit 1"

say "5 · an assistant's base inside an adopted project: the project's gate stays as the old docsys leaves it"
gate_sum() { { cat .git/hooks/pre-commit; cat .git/hooks/commit-msg 2>/dev/null || true; } | cksum; }
for who in old new; do
  make_repo "$WORK/s1-$who"
  before=$(gate_sum)
  if [ "$who" = old ]; then
    with_old docsys assistant --root asst >/dev/null 2>&1 || true
  else
    with_new docsys assistant --root asst >/dev/null || fail "S1: the new assistant failed"
  fi
  [ "$(gate_sum)" = "$before" ] || fail "S1: the $who docsys rewrote the project's gate"
  printf 'more\n' >>README.md
  g add README.md
  PATH="$([ "$who" = old ] && echo "$OLD_PATH" || echo "$NEW_PATH")" g commit -qm "after the assistant" \
    || fail "S1: the project's commit failed after the $who assistant"
done
# and on a project the new docsys moved to docsys/0.5
make_repo "$WORK/s1-05"
with_new docsys upgrade --apply --commit >/dev/null || fail "S1: the upgrade did not land"
before=$(gate_sum)
with_new docsys assistant --root asst >/dev/null || fail "S1: the assistant failed on 0.5"
[ "$(gate_sum)" = "$before" ] || fail "S1: the new docsys rewrote a 0.5 project's gate"
printf 'more\n' >>README.md
g add README.md
with_new g commit -qm "after the assistant" -m "Docs: a line" || fail "S1: the 0.5 project's commit failed after the assistant"
echo "the old and the new docsys alike leave the project's gate as it was; its next commit lands"

say "compat: all five green"
