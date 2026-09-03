#!/usr/bin/env bash
# mech/kb-ingest.sh [workdir] — flow 1 without an agent: what the binary
# guarantees around ingest by hand. Order (a page may cite the destination
# before the note arrives), relocation through `docsys raw move` (R-027),
# the record guards (R-023, the PreToolUse relay), routers (R-034/R-035),
# the frontmatter rules (R-024/R-026/R-029), and the audit record
# (R-024 body check, R-028). Every expectation is an exact string.
source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
lab_binary
F=kb-ingest
WORK=${1:-$(lab_workdir kb-ingest)}
mkdir -p "$WORK"
assert_outside_repo "$WORK"
"$LAB_DIR/fixtures/gen-kb.sh" "$WORK/kb" >/dev/null
cd "$WORK/kb"
O="$WORK/out"; mkdir -p "$O"
TODAY=$(date +%F)

say "$F · 1 seed: clean, and status counts the inbox"
expect_clean $F seed-clean .
docsys status --root . > $O/status.out
expect_in $F status-inbox "inbox: 8 note(s), oldest 2026-07-01" $O/status.out
expect_in $F status-wiki "wiki: 2 page(s), 2 unverified" $O/status.out
for f in raw/inbox/*.md; do printf '%s %s\n' "$(basename "$f")" "$(sha256_of "$f")"; done > $O/before.sha

say "$F · 2 order: a page may cite the destination before the note arrives"
mkdir -p wiki/embedded/reference
cat > wiki/embedded/reference/uart-dma.md <<MD
---
id: uart-dma
type: reference
domain: embedded
verification: unverified
updated: $TODAY
sources: [raw/embedded/2026-07-01-uart-dma-timing.md]
---
# UART DMA timing

This page states the DMA channel, baud and frame gap the F4 board's UART needs; read it before changing the ring buffer.

| setting | value |
|---|---|
| channel | DMA1_CH5 |
| baud | 115200 |
| frame gap | 3.2 ms |
MD
printf -- '- [[embedded/reference/uart-dma|UART DMA timing]] -- channel, baud, frame gap.\n' >> wiki/embedded/index.md
lint_to . $O/lint.out
expect_re $F cite-before-move '^ERROR R-059 wiki/embedded/reference/uart-dma\.md \[raw/embedded/2026-07-01-uart-dma-timing\.md\]' $O/lint.out

say "$F · 3 raw move: the note arrives, the trail resolves, the bytes are the same"
docsys raw move raw/inbox/2026-07-01-uart-dma-timing.md embedded --root . > $O/move.out 2>&1 || true
expect_in $F move-line "moved: raw/inbox/2026-07-01-uart-dma-timing.md -> raw/embedded/2026-07-01-uart-dma-timing.md" $O/move.out
expect_in $F move-no-rewrite "no page cited it" $O/move.out
expect_clean $F after-move .
expect_true $F bytes-same "sha256 of the moved note" \
  test "$(grep '^2026-07-01-uart-dma-timing.md ' $O/before.sha | cut -d' ' -f2)" = "$(sha256_of raw/embedded/2026-07-01-uart-dma-timing.md)"
expect_true $F old-path-gone "raw/inbox/2026-07-01-uart-dma-timing.md gone" test ! -e raw/inbox/2026-07-01-uart-dma-timing.md
git add -A && git commit -qm "uart-dma: page and note" 2>$O/commit.err || { cat $O/commit.err >&2; check $F commit-1 FAIL "$(cat $O/commit.err)"; }

say "$F · 4 the record is content-immutable"
printf 'one more line\n' >> raw/embedded/2026-06-20-spi.md
lint_to . $O/lint.out
expect_re $F record-edited '^ERROR R-023 raw/embedded/2026-06-20-spi\.md \[content\]' $O/lint.out
git checkout -q -- raw/embedded/2026-06-20-spi.md
rm raw/inbox/2026-07-08-lunch.md
lint_to . $O/lint.out
expect_re $F record-deleted '^ERROR R-023 raw/inbox/2026-07-08-lunch\.md \[deleted\]' $O/lint.out
git checkout -q -- raw/inbox/2026-07-08-lunch.md
expect_clean $F restored .

say "$F · 5 the relay guards the record before a byte moves"
rc=0; printf '{"tool_name":"Write","tool_input":{"file_path":"%s/raw/inbox/2026-07-08-lunch.md","content":"x"}}' "$PWD" \
  | docsys hook pre-tool-use --root . 2>$O/guard.err >/dev/null || rc=$?
expect_true $F guard-blocks "exit 2 on an existing record (got $rc)" test "$rc" = 2
expect_in $F guard-names-rule "R-023" $O/guard.err
expect_in $F guard-names-command "docsys raw move" $O/guard.err
rc=0; printf '{"tool_name":"Write","tool_input":{"file_path":"%s/raw/inbox/%s-new-note.md","content":"x"}}' "$PWD" "$TODAY" \
  | docsys hook pre-tool-use --root . 2>/dev/null >/dev/null || rc=$?
expect_true $F guard-passes-new "exit 0 on a new inbox path (got $rc)" test "$rc" = 0

say "$F · 6 routers: an unrouted page and a malformed line are named"
cp wiki/embedded/index.md $O/index.bak
grep -v 'uart-dma' $O/index.bak > wiki/embedded/index.md
lint_to . $O/lint.out
expect_re $F orphan '^WARN R-034 wiki/embedded/reference/uart-dma\.md' $O/lint.out
{ cat $O/index.bak; printf -- '- [[embedded/reference/uart-dma]] no hook\n'; } > wiki/embedded/index.md
lint_to . $O/lint.out
expect_re $F router-grammar '^WARN R-035 wiki/embedded/index\.md' $O/lint.out
mv $O/index.bak wiki/embedded/index.md
expect_clean $F routers-restored .

say "$F · 7 frontmatter: the wrong directory, an undeclared domain, missing fields"
mkdir -p wiki/embedded/howto
awk '{ if ($0 == "id: uart-dma") print "id: uart-dma-misfiled"; else print }' wiki/embedded/reference/uart-dma.md > wiki/embedded/howto/uart-dma-misfiled.md
lint_to . $O/lint.out
expect_re $F type-vs-directory '^WARN R-029 wiki/embedded/howto/uart-dma-misfiled\.md' $O/lint.out
rm wiki/embedded/howto/uart-dma-misfiled.md
awk '{ if ($0 == "id: uart-dma") print "id: uart-dma-cooking"; else if ($0 == "domain: embedded") print "domain: cooking"; else print }' wiki/embedded/reference/uart-dma.md > wiki/embedded/reference/uart-dma-cooking.md
lint_to . $O/lint.out
expect_re $F undeclared-domain '^WARN R-026 wiki/embedded/reference/uart-dma-cooking\.md' $O/lint.out
rm wiki/embedded/reference/uart-dma-cooking.md
printf -- '---\nid: bare\ntype: reference\nupdated: %s\n---\n# Bare\n\nThis page has no domain, no verification and no sources; it is here to be caught.\n' "$TODAY" > wiki/embedded/reference/bare.md
lint_to . $O/lint.out
expect_re $F missing-kb-fields '^WARN R-024 wiki/embedded/reference/bare\.md \[domain,verification,sources\]' $O/lint.out
rm wiki/embedded/reference/bare.md
expect_clean $F frontmatter-restored .

say "$F · 8 audit: the record, the body check, a revision that does not exist"
rev=$(git rev-parse --short HEAD)
awk -v rev="$rev" '{ if ($0 == "verification: unverified") { print "verification: verified"; print "verified_by: mech"; print "verified_rev: " rev } else print }' \
  wiki/embedded/reference/uart-dma.md > $O/page.tmp && mv $O/page.tmp wiki/embedded/reference/uart-dma.md
expect_clean $F verified-clean .
printf 'A line the audit never saw.\n' >> wiki/embedded/reference/uart-dma.md
lint_to . $O/lint.out
expect_re $F body-moved '^ERROR R-024 wiki/embedded/reference/uart-dma\.md \[verification\]' $O/lint.out
git checkout -q -- wiki/embedded/reference/uart-dma.md
awk '{ if ($0 == "verification: unverified") { print "verification: verified"; print "verified_by: mech"; print "verified_rev: 0000000" } else print }' \
  wiki/embedded/reference/uart-dma.md > $O/page.tmp && mv $O/page.tmp wiki/embedded/reference/uart-dma.md
lint_to . $O/lint.out
expect_re $F unknown-rev '^ERROR R-028 wiki/embedded/reference/uart-dma\.md \[verified_rev\]' $O/lint.out
git checkout -q -- wiki/embedded/reference/uart-dma.md

say "$F · 9 raw move rewrites the citing page and keeps its verification (R-027, D-077)"
mkdir -p wiki/ops/howto
cat > wiki/ops/howto/rotate-keys.md <<MD
---
id: rotate-keys
type: howto
domain: ops
verification: unverified
updated: $TODAY
sources: [raw/inbox/2026-07-15-rotate-keys.md]
---
# Rotate the deploy keys

This page is the quarterly key rotation, step by step; read it when the calendar says so.

1. Generate the new key: \`openssl rand -hex 32\`.
2. Paste it into the vault under deploy/runner.
3. Restart the runner; the old key is valid until then, not longer.
MD
printf -- '- [[ops/howto/rotate-keys|Rotate the deploy keys]] -- quarterly, three steps.\n' >> wiki/ops/index.md
git add -A && git commit -qm "rotate-keys: page" 2>$O/commit.err || check $F commit-2 FAIL "$(cat $O/commit.err)"
rev=$(git rev-parse --short HEAD)
awk -v rev="$rev" '{ if ($0 == "verification: unverified") { print "verification: verified"; print "verified_by: mech"; print "verified_rev: " rev } else print }' \
  wiki/ops/howto/rotate-keys.md > $O/page.tmp && mv $O/page.tmp wiki/ops/howto/rotate-keys.md
git add -A && git commit -qm "rotate-keys: verified" 2>$O/commit.err || check $F commit-3 FAIL "$(cat $O/commit.err)"
expect_clean $F verified-before-move .
body_before=$(awk 'f{print} /^---$/ && NR>1 {f=1}' wiki/ops/howto/rotate-keys.md)
docsys raw move raw/inbox/2026-07-15-rotate-keys.md ops --root . > $O/move.out 2>&1 || true
expect_in $F move2-line "moved: raw/inbox/2026-07-15-rotate-keys.md -> raw/ops/2026-07-15-rotate-keys.md" $O/move.out
expect_in $F move2-rewrote "rewrote: wiki/ops/howto/rotate-keys.md (1 entry)" $O/move.out
expect_in $F sources-rewritten "sources: [raw/ops/2026-07-15-rotate-keys.md]" wiki/ops/howto/rotate-keys.md
expect_in $F still-verified "verification: verified" wiki/ops/howto/rotate-keys.md
body_after=$(awk 'f{print} /^---$/ && NR>1 {f=1}' wiki/ops/howto/rotate-keys.md)
expect_true $F body-untouched "the body is byte-identical" test "$body_before" = "$body_after"
expect_true $F bytes-same-2 "sha256 of the moved note" \
  test "$(grep '^2026-07-15-rotate-keys.md ' $O/before.sha | cut -d' ' -f2)" = "$(sha256_of raw/ops/2026-07-15-rotate-keys.md)"
expect_clean $F after-move-2 .
git add -A && git commit -qm "rotate-keys: note archived" 2>$O/commit.err || check $F commit-4 FAIL "$(cat $O/commit.err)"
expect_clean $F after-move-2-committed .

say "$F · 10 raw move refuses what would lie"
rc=0; docsys raw move raw/inbox/2026-07-10-sourdough.md cooking --root . > $O/refuse.out 2>&1 || rc=$?
expect_true $F refuse-undeclared "exit 2 on an undeclared domain (got $rc)" test "$rc" = 2
expect_in $F refuse-names-r026 "R-026" $O/refuse.out
expect_true $F refuse-left-note "the note stayed" test -f raw/inbox/2026-07-10-sourdough.md
mkdir -p raw/coding && printf 'an older note with the same name\n' > raw/coding/2026-07-08-lunch.md
rc=0; docsys raw move raw/inbox/2026-07-08-lunch.md coding --root . > $O/refuse.out 2>&1 || rc=$?
expect_true $F refuse-existing "exit 2 on an existing destination (got $rc)" test "$rc" = 2
expect_in $F refuse-names-r023 "R-023" $O/refuse.out
expect_true $F refuse-kept-both "both files kept" test -f raw/inbox/2026-07-08-lunch.md -a "$(cat raw/coding/2026-07-08-lunch.md)" = "an older note with the same name"
rm raw/coding/2026-07-08-lunch.md

say "$F · 11 status after the moves"
docsys status --root . > $O/status.out
expect_in $F status-after "inbox: 6 note(s), oldest 2026-07-02" $O/status.out
expect_in $F status-verified "wiki: 4 page(s), 3 unverified" $O/status.out
summary $F
