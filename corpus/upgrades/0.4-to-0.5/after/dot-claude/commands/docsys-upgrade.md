---
description: Move this repository's docs tree to the docsys you run — the tool applies what is mechanical, you read what needs reading, the person decides
allowed-tools: Bash(docsys *), Bash(git status:*), Bash(git diff:*), Bash(git log:*), Read, Edit
---

# /docsys-upgrade — move the tree, then finish what needs a person

The tool applies what is mechanical and you read what needs reading; the
person decides whether the tree moves, each edit you propose, each line they
keep as it is, and whether the follow-ups go in a commit or a pull request.
Nothing is committed before they said yes.

1. Run `docsys upgrade --json` and show the person the notes and the plan:
   `auto` items the tool applies, `manual` items that are theirs, `info`.
   Then `docsys upgrade --apply --commit`.
2. A CI workflow its owner edited (`ci-workflow`): an `auto` item is the
   move's own — show the person its diff. A `manual` item at `file:line`
   names what to change there so the install reads the pin: propose that
   edit, and nothing else of theirs.
3. A pin listed for a re-read (`pins`): re-read it as the rules block says.
   If the page holds, run the item's `command`; if not, propose the page edit.
4. A relay, skill, command or contract its owner edited (a diff): propose
   one text that keeps the owner's lines and takes the new ones.
5. A line of the team's own text that names a retired concept
   (`retired-concepts`, at `file:line`): propose that line rewritten with the
   item's replacement, keeping the owner's other words.
6. A sha256 value is never written: a docsys/0.5 workflow checks the
   archive against its release's `SHA256SUMS`.
7. The follow-ups are described by the upgrade commit's message
   (`git log -1 --format=%B`).
8. Last, the audit: the move ended with the leftover list — what an earlier
   version left, each with its file and its fix. Walk the person through each
   item, and through `docsys lint`, until both are clean, or what is left is
   what they keep.
