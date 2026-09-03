#!/usr/bin/env bash
# gen-kb.sh <dir> — a knowledge base at the start of the ingest flow.
#
# Its own repository, root `.`, three domains, the agent layer installed and
# the character set (the survey ran on day one), two pages under
# wiki/embedded/reference/ — `spi-clock` UNFAITHFUL to its source (8 MHz
# where the note says 4 MHz), `uart-basics` faithful — and eight notes in
# raw/inbox/ (fixtures/notes/). Every commit is dated, so SHAs and R-106 are
# reproducible. Lint is clean at the tag `seed`.
source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
lab_binary
DIR=${1:?usage: gen-kb.sh <dir>}
[ -e "$DIR" ] && fail "$DIR exists"
NOTES="$LAB_DIR/fixtures/notes"

lab_git_init "$DIR"
cd "$DIR"
export DOCSYS_TODAY=2026-06-21
docsys init --profile knowledge-base --root . >/dev/null
replace_line .docmeta.yml "domains: []" "domains: [coding, embedded, ops]"
docsys agents --kb --root . >/dev/null
# the character was set on day one; the first-run survey is not this flow's subject
awk '
  /^<!-- character: unset/ { next }
  /^- Name: \(unset/    { print "- Name: Lab"; next }
  /^- Address: \(unset/ { print "- Address: by first name, informal"; next }
  /^- Tone: \(unset/    { print "- Tone: plain and brief; no humor"; next }
  { print }
' AGENTS.md > AGENTS.tmp && mv AGENTS.tmp AGENTS.md

mkdir -p wiki/coding wiki/embedded/reference wiki/ops raw/embedded
cat > wiki/index.md <<'MD'
# Knowledge base

- [[coding/index|Coding]] -- software practice.
- [[embedded/index|Embedded]] -- firmware and boards.
- [[ops/index|Ops]] -- running things.
MD
printf '# coding\n\nSoftware practice: how code is written, reviewed and shipped.\n' > wiki/coding/index.md
printf '# ops\n\nRunning things: keys, runners, deployments.\n' > wiki/ops/index.md
cat > wiki/embedded/index.md <<'MD'
# embedded

- [[embedded/reference/spi-clock|SPI clock]] -- the sensor bus clock on the F4 board.
- [[embedded/reference/uart-basics|UART basics]] -- the three UARTs and what hangs off them.
MD
printf '# Open questions\n\nProposals and discrepancies the base cannot settle by itself; one dated line each.\n' > wiki/open-questions.md

cat > raw/embedded/2026-06-20-spi.md <<'MD'
SPI1 to the pressure sensor runs at 4 MHz. Tried 8 MHz on the bench: the
sensor returned garbage above 6 MHz, so 4 MHz stays.
MD
cat > raw/embedded/2026-06-20-uart-basics.md <<'MD'
The F4 board has three UARTs. USART2 is the console at 115200 8N1; USART1
goes to the radio module; USART3 is unused and its pins are free.
MD
cat > wiki/embedded/reference/spi-clock.md <<'MD'
---
id: spi-clock
type: reference
domain: embedded
verification: unverified
updated: 2026-06-21
sources: [raw/embedded/2026-06-20-spi.md]
---
# SPI clock on the F4 board

This page states the clock of the sensor SPI bus on the F4 board; read it
before touching the prescaler.

| bus | clock |
|---|---|
| SPI1, pressure sensor | 8 MHz |
MD
cat > wiki/embedded/reference/uart-basics.md <<'MD'
---
id: uart-basics
type: reference
domain: embedded
verification: unverified
updated: 2026-06-21
sources: [raw/embedded/2026-06-20-uart-basics.md]
---
# UART basics on the F4 board

This page lists the F4 board's three UARTs and what each one is wired to;
read it before claiming a serial port.

| UART | use |
|---|---|
| USART1 | radio module |
| USART2 | console, 115200 8N1 |
| USART3 | unused, pins free |
MD
lint_clean . || fail "the base is not clean before its first commit: $(docsys lint --root . | head -5)"
dated_commit . 2026-06-21 "base: three domains, two pages, the layer" "The character was set in the first session."

# the eight notes; the connector record lands through the write gate
export DOCSYS_TODAY=2026-07-15
for n in 2026-07-01-uart-dma-timing 2026-07-02-uart-dma-timing-again 2026-07-03-uart-baud-note \
         2026-07-05-review-from-tests 2026-07-08-lunch 2026-07-10-sourdough 2026-07-15-rotate-keys; do
  cp "$NOTES/$n.md" "raw/inbox/$n.md"
done
docsys inbox add --source git --id ledgerkit@3f9c2a1b7d4e \
  --title "ledgerkit: fix(sync): replay buffer is 4096 entries" --date 2026-07-12 \
  "$NOTES/2026-07-12-git-ledgerkit-sync-buffer.md" --root . > .landed
landed=$(sed -n 's/^captured: //p' .landed); rm .landed
[ -n "$landed" ] || fail "the connector record did not land"
mv "$landed" raw/inbox/2026-07-12-git-ledgerkit-sync-buffer.md   # not yet a record: uncommitted (D-031)
lint_clean . || fail "the base is not clean with its inbox: $(docsys lint --root . | head -5)"
dated_commit . 2026-07-15 "inbox: eight notes" "Seven notes by hand, one record from the git connector."
git tag seed
unset DOCSYS_TODAY
printf 'gen-kb: %s at %s (seed)\n' "$DIR" "$(git rev-parse --short HEAD)"
