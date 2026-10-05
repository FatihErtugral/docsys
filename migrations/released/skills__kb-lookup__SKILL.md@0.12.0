---
name: kb-lookup
description: Answer from the knowledge base — "what do my notes say about X", "check my brain for X", "did I write anything about X". Read-only; answers with sources or says it is not there.
---

# kb-lookup — the read gate

Read-only. Never write, never fix what you find; report gaps instead.

1. `docsys lookup <words> --root <base>` — the mechanical first hop: every
   page, local and consumed (`@namespace/id`), that names all the words,
   scored by where they occur. Read the page it points at; a consumed page
   is another tree's contract and is cited as `@namespace/id`.
2. Nothing? `wiki/index.md` → the domain → `wiki/<domain>/index.md` → the
   page; then grep `wiki/` for tags and headings.
3. Still nothing → **say it is not in the base.** Never answer from your own
   knowledge while implying the base said it; offer to capture the question.
4. Answer WITH the page path, and say plainly when the page is
   `unverified` — an unaudited page may be wrong, and the reader decides how
   much to lean on it.

`raw/` is evidence, not an answer: quote it only to show where a page came
from.
