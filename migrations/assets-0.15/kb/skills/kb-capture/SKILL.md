---
name: kb-capture
description: Save something into the knowledge base — "note this", "remember this", "add to my brain", "log this lesson". Writes to raw/inbox/ with zero classification; sorting is ingest's job.
---

# kb-capture — the write gate

Capture costs nothing or it does not happen. Never classify here, never ask
where it belongs, never open a wiki page.

1. Write ONE file to `raw/inbox/<YYYY-MM-DD>-<short-kebab-slug>.md`.
2. Content: the note in the user's own words, plus one line naming **why it is
   worth keeping** — that line is what makes it distillable later. Add a
   `suggested-domain:` line only if it is obvious; ingest decides.
3. Confirm in one sentence. Do not run lint, do not touch `wiki/`.

Never paraphrase away a specific: a number, a version, an error string, a
command is the part that will be worth having.
