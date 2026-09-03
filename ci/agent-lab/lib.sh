#!/usr/bin/env bash
# ci/agent-lab/lib.sh — shared by every lab script: bash + git + coreutils +
# grep + awk. No jq, no `sed -i` (BSD sed takes a suffix), no network.
# Sourced, never run. The same conventions as ci/e2e.sh.
set -euo pipefail

LAB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$LAB_DIR/../.." && pwd)"
OUT_DIR="${LAB_OUT:-$LAB_DIR/out}"

# Fixtures never live inside this repository: a tool that finds an enclosing
# repository may skip `git init`, and a `git add -A` in a fixture would then
# stage the lab's own checkout. Work directories come from lab_workdir
# (outside every repository); out/ holds artifacts and copies only.
lab_workdir() { # lab_workdir <label> — a fresh directory outside any repository
  local base="${LAB_WORK:-${TMPDIR:-/tmp}}"
  mkdir -p "$base"
  local d; d=$(mktemp -d "$base/docsys-lab-$1-XXXXXX")
  assert_outside_repo "$d"
  printf '%s' "$d"
}
assert_outside_repo() { # assert_outside_repo <dir>
  local top; top=$(git -C "$1" rev-parse --show-toplevel 2>/dev/null || true)
  [ -z "$top" ] || fail "$1 is inside the repository $top — fixtures must not nest in a repository"
}

say()  { printf '\n== %s\n' "$*"; }
fail() { printf 'LAB FAIL: %s\n' "$*" >&2; exit 1; }

# The binary under test is the one this checkout built; the installed one is
# never what the lab measures.
lab_binary() {
  # LAB_BIN_DIR: a build elsewhere (a candidate binary while sessions still run on the old one)
  local bin="${LAB_BIN_DIR:-$REPO_DIR/target/release}"
  if [ -x "$bin/docsys" ]; then
    export PATH="$bin:$PATH"
  fi
  command -v docsys >/dev/null || fail "docsys not on PATH — cargo build --release first"
  command -v git >/dev/null || fail "git not on PATH"
}

# A fixture's git identity: fixed, so every SHA is reproducible.
lab_git_identity() {
  git config user.email builder@example.invalid
  git config user.name builder
  git config commit.gpgsign false
  git config tag.gpgsign false
}

# lab_git_init <dir> — a repository on `main`, identity set.
lab_git_init() {
  mkdir -p "$1"
  git -C "$1" init -q
  git -C "$1" symbolic-ref HEAD refs/heads/main
  ( cd "$1" && lab_git_identity )
}

# dated_commit <dir> <YYYY-MM-DD> <subject> [body] — author and committer
# dated at noon UTC of that day, so `git log --format=%cs` and a page's
# `updated:` agree (R-106) and the SHA is a pure function of the inputs.
dated_commit() {
  local dir=$1 date=$2 subject=$3 body=${4:-}
  local stamp="${date}T12:00:00+00:00"
  (
    cd "$dir"
    git add -A
    if [ -n "$body" ]; then
      GIT_AUTHOR_DATE="$stamp" GIT_COMMITTER_DATE="$stamp" git commit -q --allow-empty -m "$subject" -m "$body"
    else
      GIT_AUTHOR_DATE="$stamp" GIT_COMMITTER_DATE="$stamp" git commit -q --allow-empty -m "$subject"
    fi
  )
}

# dated_tag <dir> <YYYY-MM-DD> <tag> — an annotated tag with its own date.
dated_tag() {
  local stamp="${2}T12:00:00+00:00"
  ( cd "$1" && GIT_COMMITTER_DATE="$stamp" git tag -a "$3" -m "$3" )
}

# sha_of <dir> <grep-pattern> — the short sha of the newest commit whose
# subject matches; empty when none.
sha_of() {
  git -C "$1" log --format=%h --grep="$2" -n 1
}

# The last summary line of lint: `-- N error(s), M warning(s)`.
lint_summary() {
  (docsys lint --root "$1" ${2:+--repo "$2"} || true) | grep -- '^-- ' | tail -1
}

# lint_clean <root> [repo] — zero errors AND zero warnings (the strict rule).
lint_clean() {
  case "$(lint_summary "$1" "${2:-}")" in "-- 0 error(s), 0 warning(s)"*) return 0 ;; *) return 1 ;; esac
}

# Results: one TSV line per check. `RESULTS` is set by the runner; a script
# run by hand prints to stderr only.
RESULTS="${RESULTS:-/dev/null}"
FAILED=0
PASSED=0
check() { # check <flow> <name> <PASS|FAIL> <evidence>
  local evidence
  evidence=$(printf '%s' "$4" | tr '\n\t' '| ' | head -c 400)
  printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$evidence" >> "$RESULTS"
  if [ "$3" = PASS ]; then
    PASSED=$((PASSED + 1))
  else
    FAILED=$((FAILED + 1))
    printf 'FAIL  %s / %s — %s\n' "$1" "$2" "$evidence" >&2
  fi
}
# expect_in <flow> <name> <fixed-string> <file> — the string is in the file.
expect_in() {
  if grep -qF -- "$3" "$4"; then
    check "$1" "$2" PASS "$3"
  else
    check "$1" "$2" FAIL "missing \`$3\` in $4 :: $(head -c 300 "$4")"
  fi
}
# expect_absent <flow> <name> <fixed-string> <file>
expect_absent() {
  if grep -qF -- "$3" "$4"; then
    check "$1" "$2" FAIL "unexpected \`$3\` in $4"
  else
    check "$1" "$2" PASS "no \`$3\`"
  fi
}
# expect_true <flow> <name> <evidence> <command…> — the command exits 0.
expect_true() {
  local flow=$1 name=$2 evidence=$3; shift 3
  if "$@"; then check "$flow" "$name" PASS "$evidence"; else check "$flow" "$name" FAIL "$evidence"; fi
}
# expect_clean <flow> <name> <root> [repo]
expect_clean() {
  local s; s=$(lint_summary "$3" "${4:-}")
  case "$s" in "-- 0 error(s), 0 warning(s)"*) check "$1" "$2" PASS "$s" ;; *)
    check "$1" "$2" FAIL "$s :: $( (docsys lint --root "$3" ${4:+--repo "$4"} || true) | grep -E '^(ERROR|WARN)' | head -5)"
  ;; esac
}
summary() { # summary <flow>
  printf '\n%s: %d passed, %d failed\n' "$1" "$PASSED" "$FAILED"
  [ "$FAILED" -eq 0 ]
}

# sha256 of a file, portable (sha256sum on Linux, shasum on macOS).
sha256_of() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# replace_line <file> <exact-old-line> <new-line> — awk, no sed -i.
replace_line() {
  awk -v old="$2" -v new="$3" '{ if ($0 == old) print new; else print }' "$1" > "$1.tmp" && mv "$1.tmp" "$1"
}

# expect_re <flow> <name> <extended-regex> <file> — a line matches.
expect_re() {
  if grep -Eq -- "$3" "$4"; then
    check "$1" "$2" PASS "$3"
  else
    check "$1" "$2" FAIL "no line matching /$3/ in $4 :: $(head -c 300 "$4")"
  fi
}
# lint_to <root> <file> — lint output to a file (never a pipe: grep -q would close it early).
lint_to() { (docsys lint --root "$1" || true) > "$2"; }
# set_field <file> <key> <value> — replace `key: …` in the frontmatter, or add it after `verification:`.
set_field() {
  awk -v key="$2" -v val="$3" '
    BEGIN { fm = 0; done = 0 }
    NR == 1 && $0 == "---" { fm = 1; print; next }
    fm == 1 && $0 == "---" { if (!done) { } fm = 2; print; next }
    fm == 1 && index($0, key ":") == 1 { print key ": " val; done = 1; next }
    { print }
  ' "$1" > "$1.tmp" && mv "$1.tmp" "$1"
}

# bump_updated <file> <YYYY-MM-DD> — the `updated:` field, as the tooling would (R-052).
bump_updated() {
  awk -v d="$2" '{ if (index($0, "updated:") == 1) print "updated: " d; else print }' "$1" > "$1.tmp" && mv "$1.tmp" "$1"
}
