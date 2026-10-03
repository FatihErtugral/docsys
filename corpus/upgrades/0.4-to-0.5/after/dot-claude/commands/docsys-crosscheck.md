---
description: Read pages against their sources and the code — the pages the person names, or every page changed since a revision; correct what is wrong in one commit, record what cannot be settled
allowed-tools: Bash(docsys *), Bash(git add:*), Bash(git commit:*), Bash(git diff:*), Bash(git log:*), Bash(git show:*), Bash(git status:*), Read, Grep, Glob, Edit
---

# /docsys-crosscheck [<page>… | <ref>] — read pages against their evidence

The person starts it; you read, correct and record, then stop. The evidence
is what a page names in `sources:` and the code its pins resolve to now.

1. The pages: the ones the person named, or every page changed since the
   revision they gave (a branch, a tag, a commit). Given neither, ask which.
2. Run `docsys crosscheck <page>…` or `docsys crosscheck --since <ref>`: each
   page, its `sources:`, and each pin's code lines now —
   `path:L<first>-L<last>`, a whole file, or why the pin does not resolve.
   With `--since`, a consumed source that moved since then is marked so, and
   its page is read even when the page itself did not change.
3. Read each page whole, then every statement in it against those sources
   and lines. Follow a source to what it names; read beyond a pin's lines
   only where they leave a statement open.
4. A statement the code or a source contradicts: correct that statement,
   and nothing the evidence does not touch.
5. What you cannot settle — a statement the evidence neither confirms nor
   contradicts, two sources that disagree, a pin that no longer resolves
   while the code does not say which side is wrong: record it as the rules
   block says, `docsys question add` or `docsys debt add` with
   `--topic <page-id>`, and leave the page's words as they are.
6. The corrections go in one ordinary commit whose message says what was
   corrected on which page. Nothing marks a page as cross-checked: write no
   verification field, trailer or record, in the page or anywhere else.
7. End with a short report: the pages read, what was corrected, and each
   question or debt item recorded.
