---
name: docsys-export
description: Produce an audience-shaped document (end-user, developer, designer, …) from the docs tree — "create the end-user doc for feature X", "make a user guide", "export the designer spec", "translate the product doc". Runs docsys export, closes audience gaps by authoring pages with approval, handles language intent.
---

# docsys-export — audience-shaped documents

The binary selects and composes; it never writes prose. YOU author what is
missing — with approval — and the tree keeps it fresh afterwards.

## 1. Discover

`docsys export plan --root docs --audience <a>` — what exists for this reader.
An error naming the whole-tree gap means the pages do not exist yet: go to §3.
The vocabulary is the tree's own (`audiences:` in `.docmeta.yml`); an
undeclared page reads as `developer` (D-033).

## 2. Compose

- One feature: `docsys export feature <id>… [--follow] --audience <a>
  --title "…" --root docs --out <file>`
- Whole product: author or update the product map (a markdown file OUTSIDE the
  docs root), then `docsys export product <map> --audience <a> --root docs
  --out <file>`

Read every WARN: `gap:` lines name related pages this reader cannot use yet;
an `unchanged — left untouched` result is success, not failure. A refusal
listing wrong-audience pages means §3, never `--force`-style workarounds.

## 3. Close a gap (the only judgment step)

For each missing page: distil from the EXISTING pages — invent nothing; if a
fact exists nowhere in the tree, ask, do not guess. The voice matches the
reader: an end-user page never names source files, classes, or tests. Give it
the tree's usual frontmatter plus `audience: <a>`, and gate with `docsys lint --root docs` until clean. **Show the first page and get
approval before authoring the rest.**

## 4. Language

`--lang <code>` states the document's language; WARNs name the pages declared
otherwise. Translating a page is editing that page: its structure stays, and
P/R-123 decides each term — when unsure whether something is a proper name,
keep it.
Re-run the export afterwards: the per-page stamps changed only where content
did, so only those sections needed the work.
