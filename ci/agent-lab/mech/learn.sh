#!/usr/bin/env bash
# mech/learn.sh [workdir] — flow 3 without an agent: a base learns from
# projects. `consume discover`, `docsys assistant` in one command (and
# idempotent), the git connector's arithmetic (`--since`, `--limit`,
# `--all`, `--as`, bookkeeping skipped, nothing landed twice), the record
# shape, the `@namespace/id` lifecycle (clean after fetch, R-024 when the
# source moves, status counting it), `forget` on a cited record, and
# `lookup`. Exact strings throughout.
source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
lab_binary
F=learn
WORK=${1:-$(lab_workdir learn)}
mkdir -p "$WORK"
assert_outside_repo "$WORK"
"$LAB_DIR/fixtures/gen-estate.sh" "$WORK/estate" >/dev/null
O="$WORK/out"; mkdir -p "$O"
E="$WORK/estate"
TODAY=$(date +%F)

say "$F · 1 discover, then one command"
mkdir -p "$WORK/probe" && ( cd "$WORK/probe" && docsys init --profile knowledge-base --root . >/dev/null )
docsys consume discover "$E" --root "$WORK/probe" > "$O/discover.out" 2>&1 || true
expect_in $F discover-count "-- 4 candidate(s); nothing written" "$O/discover.out"
for p in relay ledger gateway; do expect_in $F "discover-$p" "$p" "$O/discover.out"; done
expect_re $F discover-names-profile "^notebook[[:space:]]+knowledge-base[[:space:]]" "$O/discover.out"
docsys assistant --root "$WORK/hub" --projects "$E" --domains coding --since 2026-01-01 > "$O/assistant.out" 2>&1 || check $F assistant FAIL "$(cat "$O/assistant.out")"
expect_in $F assistant-created "base: created" "$O/assistant.out"
for p in relay ledger gateway; do expect_in $F "assistant-consume-$p" "consume: $p" "$O/assistant.out"; done
expect_in $F assistant-skips-base "skipped notebook" "$O/assistant.out"
expect_in $F assistant-records-relay "records: relay — 2 new commit record(s), 0 already there" "$O/assistant.out"
expect_in $F assistant-records-ledger "records: ledger — 1 new commit record(s), 0 already there" "$O/assistant.out"
expect_in $F assistant-records-gateway "records: gateway — 2 new commit record(s), 0 already there" "$O/assistant.out"
cd "$WORK/hub"
expect_true $F hub-layer "hooks installed" test -x .claude/hooks/pre-commit-docs.sh
expect_true $F hub-agents-md "AGENTS.md names the sources beyond the inbox" grep -q 'Sources beyond the inbox' AGENTS.md
expect_clean $F hub-clean .
docsys assistant --root "$WORK/hub" --projects "$E" --since 2026-01-01 > "$O/assistant2.out" 2>&1 || true
expect_in $F assistant-idempotent-base "base: kept" "$O/assistant2.out"
expect_in $F assistant-idempotent-records "records: relay — 0 new commit record(s), 2 already there" "$O/assistant2.out"
docsys status --root . > "$O/status.out"
expect_in $F status-inbox "inbox: 5 note(s)" "$O/status.out"
expect_in $F status-consumed "consumed: gateway 2 page(s) fetched $TODAY · ledger 2 page(s) fetched $TODAY · relay 2 page(s) fetched $TODAY" "$O/status.out"

say "$F · 2 the record shape"
rec=$(ls raw/inbox/*-relay-*backoff*.md 2>/dev/null | head -1)
expect_true $F record-exists "a relay record ($rec)" test -n "$rec"
expect_re $F record-source '^source: git$' "$rec"
expect_re $F record-source-id '^source_id: relay@[0-9a-f]{12}$' "$rec"
expect_in $F record-title 'title: "relay: relay: backoff doubles from 200 ms"' "$rec"
expect_re $F record-commit-line '^commit [0-9a-f]{40} in relay, 2026-06-02$' "$rec"
expect_in $F record-files "- src/lib.rs" "$rec"
expect_in $F record-body "Measured: 4 attempts finish under 3.2 s." "$rec"
expect_in $F record-why "Why it is worth keeping:" "$rec"

say "$F · 3 connector arithmetic"
rm -rf "$WORK/arith" && mkdir -p "$WORK/arith" && ( cd "$WORK/arith" && docsys init --profile knowledge-base --root . >/dev/null )
docsys inbox pull "$E/relay" --since 2026-01-01 --limit 2 --root "$WORK/arith" > "$O/pull.out" 2>&1 || true
expect_true $F limit-two "exactly 2 captured" test "$(grep -c '^captured: raw/inbox/' "$O/pull.out")" = 2
expect_absent $F limit-no-bookkeeping "touch-run-relay" "$O/pull.out"
docsys inbox pull "$E/relay" --since 2026-01-01 --limit 2 --root "$WORK/arith" > "$O/pull2.out" 2>&1 || true
expect_true $F pull-idempotent "2 already captured" test "$(grep -c '^already captured: raw/inbox/' "$O/pull2.out")" = 2
docsys inbox pull "$E/relay" --since 2026-01-01 --all --root "$WORK/arith" > "$O/pull3.out" 2>&1 || true
expect_true $F all-adds-bookkeeping "3 more with --all" test "$(grep -c '^captured: raw/inbox/' "$O/pull3.out")" = 3
docsys inbox pull "$E/relay" --since 2026-06-03 --root "$WORK/arith" --as rl > "$O/pull4.out" 2>&1 || true
expect_true $F since-narrows "nothing worth reading after 2026-06-03" test "$(grep -c '^captured: ' "$O/pull4.out")" = 0
docsys inbox pull "$E/gateway" --since 2026-01-01 --root "$WORK/arith" --as gw > "$O/pull5.out" 2>&1 || true
expect_true $F as-namespace "records under gw@" sh -c "grep -l '^source_id: gw@' $WORK/arith/raw/inbox/*.md >/dev/null"

say "$F · 4 the @namespace/id lifecycle"
cd "$WORK/hub"
mkdir -p wiki/coding/explanation
cat > wiki/coding/explanation/relay-in-one-page.md <<MD
---
id: relay-in-one-page
type: explanation
domain: coding
verification: unverified
updated: $TODAY
sources: [@relay/retry-policy]
---
# Relay in one page

This page explains what relay promises a caller, learned from relay's own reference; read it before depending on relay.

Four attempts, then a dead letter.
MD
printf '# coding\n\n- [[coding/explanation/relay-in-one-page|Relay in one page]] -- the promise, learned from relay.\n' > wiki/coding/index.md
grep -q 'coding/index' wiki/index.md || printf -- '- [[coding/index|Coding]] -- code.\n' >> wiki/index.md
expect_clean $F cites-fetched-page .
git add -A && git commit -qm "learned from relay" 2>"$O/commit.err" || check $F commit-1 FAIL "$(cat "$O/commit.err")"
rev=$(git rev-parse --short HEAD)
awk -v rev="$rev" '{ if ($0 == "verification: unverified") { print "verification: verified"; print "verified_by: mech"; print "verified_rev: " rev } else print }' \
  wiki/coding/explanation/relay-in-one-page.md > "$O/page.tmp" && mv "$O/page.tmp" wiki/coding/explanation/relay-in-one-page.md
expect_clean $F verified-against-fetched .
git add -A && git commit -qm "verified" 2>/dev/null
( cd "$E/relay" && awk '{ if (index($0, "Four attempts, exponential backoff") == 1) print "Six attempts, exponential backoff starting at 200 ms, then a dead letter."; else print }' docs/reference/retry-policy.md > r.tmp && mv r.tmp docs/reference/retry-policy.md && dated_commit . 2026-08-01 "relay: six attempts" "Four was not enough for the slow dependency; six, measured, finishes under 13 s." )
docsys fetch --root . >/dev/null 2>&1 || true
lint_to . "$O/lint.out"
expect_re $F source-moved '^ERROR R-024 wiki/coding/explanation/relay-in-one-page\.md \[@relay/retry-policy\]' "$O/lint.out"
docsys status --root . > "$O/status.out"
expect_in $F status-sources-moved "sources: 1 verified page(s) whose consumed sources moved since verification" "$O/status.out"
docsys inbox pull "$E/relay" --since 2026-01-01 --root . > "$O/pull6.out" 2>&1 || true
expect_true $F new-commit-lands "the six-attempts commit lands" grep -q '^captured: raw/inbox/.*six-attempts' "$O/pull6.out"
# D-077's order: the hard gate refuses a commit while a verified page is stale; the page goes back
# to unverified first, the fetch and the record land, then another audit verifies against the new baseline
rc=0; git add -A && git commit -qm "relay moved: fetched, pulled" 2>"$O/commit.err" || rc=$?
expect_true $F gate-refuses-stale-verified "the gate refuses the commit while the verified page is stale (exit $rc)" test "$rc" != 0
awk '{ if (index($0, "verification: verified") == 1) print "verification: unverified"; else if (index($0, "verified_by:") == 1 || index($0, "verified_rev:") == 1) next; else print }' \
  wiki/coding/explanation/relay-in-one-page.md > "$O/page.tmp" && mv "$O/page.tmp" wiki/coding/explanation/relay-in-one-page.md
expect_clean $F unverified-again .
git add -A && git commit -qm "relay moved: fetched, pulled; the page back to unverified" 2>"$O/commit.err" || check $F commit-fetch FAIL "$(cat "$O/commit.err")"
rev=$(git rev-parse --short HEAD)
awk -v rev="$rev" '{ if ($0 == "verification: unverified") { print "verification: verified"; print "verified_by: mech"; print "verified_rev: " rev } else print }' \
  wiki/coding/explanation/relay-in-one-page.md > "$O/page.tmp" && mv "$O/page.tmp" wiki/coding/explanation/relay-in-one-page.md
expect_clean $F re-verified .

say "$F · 5 forgetting a cited record is refused; the page first"
rec=$(ls raw/inbox/*-relay-*backoff*.md | head -1)
cat > wiki/coding/explanation/backoff.md <<MD
---
id: backoff
type: explanation
domain: coding
verification: unverified
updated: $TODAY
sources: [$rec]
---
# Why relay's backoff doubles

This page explains why relay doubles its backoff from 200 ms; read it before tuning the retry.

A fixed 200 ms retry hammered the dependency in May; doubling gives it room, and four attempts finish under 3.2 s.
MD
printf -- '- [[coding/explanation/backoff|Why the backoff doubles]] -- from relay'"'"'s history.\n' >> wiki/coding/index.md
git add -A && git commit -qm "backoff page" 2>/dev/null
rc=0; docsys forget "$rec" --reason "test" --root . > "$O/forget.out" 2>&1 || rc=$?
expect_true $F forget-record-refused "exit $rc" test "$rc" != 0
expect_in $F forget-names-page "forget the page first" "$O/forget.out"
docsys forget backoff --reason "learned it elsewhere" --root . > "$O/forget2.out" 2>&1 || check $F forget-page FAIL "$(cat "$O/forget2.out")"
docsys forget "$rec" --reason "with its page" --root . > "$O/forget3.out" 2>&1 || check $F forget-record FAIL "$(cat "$O/forget3.out")"
expect_true $F record-under-forgotten "raw/_forgotten/" test -f "raw/_forgotten/inbox/$(basename "$rec")"
expect_clean $F after-forget .
docsys inbox pull "$E/relay" --since 2026-01-01 --root . > "$O/pull7.out" 2>&1 || true
expect_true $F forgotten-never-lands-again "already captured under raw/_forgotten/" grep -q '^already captured: raw/_forgotten/' "$O/pull7.out"

say "$F · 6 lookup: the first hop across what the base consumes"
docsys lookup retry --root . > "$O/lookup.out" 2>&1 || true
expect_in $F lookup-consumed "@relay/retry-policy" "$O/lookup.out"
rc=0; docsys lookup zzznothing --root . > "$O/lookup2.out" 2>&1 || rc=$?
expect_true $F lookup-honest "exit 1 when nothing names the words (got $rc)" test "$rc" = 1
summary $F
