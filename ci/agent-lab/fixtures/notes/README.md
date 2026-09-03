# The inbox notes

Eight invented notes a knowledge base's inbox holds at the start of the
ingest flow (fixture `gen-kb.sh`). `expected.tsv` says where each belongs
and what it stresses; `tokens.tsv` lists the strings that must survive a
distillation character for character (rubric row A2).

| note | stress |
|---|---|
| `2026-07-01-uart-dma-timing.md` | a table of measured values — every token verbatim |
| `2026-07-02-uart-dma-timing-again.md` | a duplicate with one extra row — one page, two sources |
| `2026-07-03-uart-baud-note.md` | contradicts the first note (9600 vs 115200) — surfaced, never silently resolved |
| `2026-07-05-review-from-tests.md` | steps AND a why — two pages (R-031), never one |
| `2026-07-08-lunch.md` | noise — stays in the inbox with a line in open questions, never deleted |
| `2026-07-10-sourdough.md` | fits no declared domain — stays, a domain is proposed, never invented |
| `2026-07-12-git-ledgerkit-sync-buffer.md` | a connector record (landed through `docsys inbox add`) — provenance respected, the number kept |
| `2026-07-15-rotate-keys.md` | carries `suggested-domain: ops` and a command — the command verbatim |

`disposition`: `page` — one new page; `merge` — the same page as the note
above it, both in `sources:`; `conflict` — a contradiction to surface;
`split` — two pages of two types; `stay` — the note remains in `raw/inbox/`.
