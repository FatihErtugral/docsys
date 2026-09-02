#!/usr/bin/env bash
# End-to-end: a fresh machine, `docsys` on PATH, every first-run flow exercised
# with real paths. Run it anywhere (CI does; a container reproduces a clean
# box: `docker run --rm -v "$PWD":/src -w /tmp rust:1-bookworm bash -c
# "cargo install --path /src -q && /src/ci/e2e.sh"`).
set -euo pipefail

say()  { printf '\n== %s\n' "$*"; }
fail() { printf 'E2E FAIL: %s\n' "$*" >&2; exit 1; }

command -v docsys >/dev/null || fail "docsys not on PATH"
command -v git >/dev/null || fail "git not on PATH"
git config --global user.email >/dev/null 2>&1 || git config --global user.email e2e@example.invalid
git config --global user.name  >/dev/null 2>&1 || git config --global user.name e2e
git config --global init.defaultBranch main >/dev/null 2>&1 || true

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

say "1 · project: adopt on a fresh repository"
mkdir app && cd app && git init -q
docsys adopt >/dev/null
git add -A && git commit -qm init
docsys lint --root docs | grep -q -- '-- 0 error(s)' || fail "fresh adopt does not lint clean"

say "2 · doctor: the pipeline proves itself alive"
docsys doctor --repo . --root docs | tee /tmp/doctor.out | grep -q -- '-- pipeline alive' \
  || fail "doctor on a fresh adopt: $(cat /tmp/doctor.out)"

say "3 · doctor: a dead gate is named, not missed"
mv .git/hooks/pre-commit /tmp/gate.bak
printf '#!/bin/sh\nexec true\ndocsys lint --root docs\n' > .git/hooks/pre-commit
out=$(docsys doctor --repo . --root docs || true)   # doctor exits 1 here BY DESIGN
grep -q 'dead code' <<<"$out" || fail "dead gate not detected: $out"
mv /tmp/gate.bak .git/hooks/pre-commit

say "4 · gate: the code-without-docs question"
echo 'fn main() {}' > main.rs && git add main.rs
docsys gate --repo . --root docs | grep -q '^GATE ' || fail "gate misses staged code without docs"
git add docs 2>/dev/null; printf -- '- [ ] 2026-08-16 wire CI -- deferred: e2e -- repay when: v1\n' >> docs/work/debt.md
git add docs/work/debt.md
docsys gate --repo . --root docs | grep -q '^GATE ' && fail "gate fires although docs are staged"
git commit -qm "code with docs"
cd "$WORK"

say "5 · knowledge base: two commands from zero"
mkdir brain && cd brain && git init -q
docsys init --profile knowledge-base --root . >/dev/null
docsys agents --kb --root . >/dev/null
test -f .claude/skills/kb-capture/SKILL.md || fail "kb skills missing"
docsys lint --root . | grep -q -- '-- 0 error(s), 0 warning(s)' || fail "fresh kb tree not clean"
cd "$WORK"

say "6 · federation: two git providers, one estate, one document"
for svc in auth billing; do
  mkdir -p "$svc/docs/howto" && (cd "$svc" && git init -q)
  cat > "$svc/docs/.docmeta.yml" <<EOF
spec: docsys/0.4
profile: project
default_content_language: en
EOF
  printf '# docs\n\n- [[howto/use-%s|Use %s]] -- Using it.\n' "$svc" "$svc" > "$svc/docs/index.md"
  cat > "$svc/docs/howto/use-$svc.md" <<EOF
---
id: use-$svc
type: howto
updated: 2026-08-16
---

# Using $svc

The $svc service in one page.
EOF
  (cd "$svc" && docsys export manifest --root docs --out docs/manifest.docsys >/dev/null 2>&1 \
    && git add -A && git commit -qm docs)
done
mkdir -p estate/docs && cd estate
cat > docs/.docmeta.yml <<EOF
spec: docsys/0.4
profile: project
default_content_language: en
consume_base: "file://$WORK/{ns}#docs"
consume: [auth, billing]
EOF
printf '# estate\n' > docs/index.md
docsys fetch --root docs | grep -c 'page(s)' | grep -q 2 || fail "fetch did not cover both providers"
docsys export feature @auth/use-auth @billing/use-billing \
  --title "Estate guide" --root docs --out guide.md 2>/dev/null
grep -q 'The auth service' guide.md && grep -q 'The billing service' guide.md \
  || fail "cross-repo composition incomplete"
docsys fetch --root docs | grep -q 'unchanged, skipped' || fail "manifest skip did not engage"
docsys export feature @auth/use-auth @billing/use-billing \
  --title "Estate guide" --root docs --out guide.md 2>/tmp/unch.err
grep -q 'unchanged' /tmp/unch.err || fail "unchanged output was rewritten"
cd "$WORK"

say "7 · export: audience refusal is honest"
cd app
docsys export feature ghost-id --root docs --out /dev/null 2>/tmp/ref.err && fail "unknown id composed"
grep -q 'no permanent page' /tmp/ref.err || fail "refusal does not name the problem"

say "8 · freshness: a pin catches drift, history dates the page, the range gate asks CI's question"
mkdir -p docs/reference
cat > docs/reference/entry.md <<EOF
---
id: entry
type: reference
updated: $(date +%F)
---
# Entry

This page states what the entry point prints; read it before changing main.

It prints nothing.
EOF
printf -- '- [[reference/entry|Entry]] -- what main prints.\n' >> docs/index.md
docsys pin reference/entry main.rs >/dev/null || fail "pin did not land"
grep -q 'sha256:' docs/reference/entry.md || fail "pin wrote no hash"
git add -A && git commit -qm "entry page, pinned"
docsys lint --root docs | grep -q -- '-- 0 error(s)' || fail "a fresh pin is not clean"
echo 'fn main() { println!("hi") }' > main.rs
(docsys lint --root docs || true) | grep -q 'R-111' || fail "a moved region is not reported"   # lint exits 1 here BY DESIGN
docsys pin --refresh reference/entry >/dev/null || fail "refresh failed"
docsys lint --root docs | grep -q -- '-- 0 error(s)' || fail "refreshed pin is not clean"
git add -A && git commit -qm "main moved, page re-read"
base=$(git rev-parse HEAD)
echo 'pub fn helper() {}' > lib.rs      # code the pin does not cover: lint stays clean, only the range question is open
git add -A && git commit -qm "code only"
docsys gate --repo . --root docs --range "$base..HEAD" >/dev/null && fail "range gate passed code without docs"
(docsys gate --repo . --root docs --range "$base..HEAD" || true) | grep -q '^GATE ' || fail "range gate names nothing"

say "9 · compile: a howto becomes a skill, and goes stale with its page"
mkdir -p docs/howto
cat > docs/howto/ship.md <<EOF
---
id: ship
type: howto
updated: $(date +%F)
---
# Ship

This page lists the shipping steps; read it before tagging.

1. Run the tests.
2. Tag the commit.
EOF
printf -- '- [[howto/ship|Ship]] -- the shipping steps.\n' >> docs/index.md
docsys compile ship --root docs >/dev/null || fail "compile refused a howto"
grep -q 'docsys_source_hash: sha256:' .claude/skills/ship/SKILL.md || fail "skill carries no source hash"
grep -q '^2. Tag the commit.$' .claude/skills/ship/SKILL.md || fail "skill body is not the page"
docsys lint --root docs | grep -q -- '-- 0 error(s)' || fail "a fresh compile is not clean"
printf '3. Push the tag.\n' >> docs/howto/ship.md
(docsys lint --root docs || true) | grep -q 'R-095' || fail "a moved howto did not stale its skill"   # lint exits 1 here BY DESIGN
docsys compile ship --root docs >/dev/null
docsys lint --root docs | grep -q -- '-- 0 error(s)' || fail "recompiled skill is not clean"
docsys compile entry --root docs >/dev/null 2>&1 && fail "a reference page compiled"
cd "$WORK"

say "10 · lookup and consume: a question's first hop across what a tree consumes"
mkdir hub && cd hub && git init -q && docsys init --root docs >/dev/null
docsys consume add "$WORK/auth" --root docs | grep -q 'auth' || fail "consume add did not register the provider"
docsys consume add "$WORK/auth" --root docs >/dev/null 2>&1 && fail "a provider was consumed twice"
docsys fetch --root docs >/dev/null || fail "fetch after consume add failed"
docsys lookup auth service --root docs | grep -q '@auth/use-auth' || fail "lookup did not find the consumed page"
docsys lookup nothing-like-this --root docs >/dev/null 2>&1 && fail "lookup found a page for nonsense"
docsys consume discover "$WORK" --root docs | grep -q 'billing' || fail "discover did not list the other provider"
docsys consume discover "$WORK" --root docs | grep -q 'already consumed' || fail "discover did not mark the consumed one"
cd "$WORK"

printf '\nE2E OK\n'
