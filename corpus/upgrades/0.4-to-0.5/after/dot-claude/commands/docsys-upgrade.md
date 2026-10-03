---
description: Move this repository's docs tree to the docsys you run — the tool applies what is mechanical, you read what needs reading, the person decides
allowed-tools: Bash(docsys *), Bash(git status:*), Bash(git diff:*), Bash(git log:*), Read, Edit
---

# /docsys-upgrade — move the tree, then finish what needs a person

The tool applies what is mechanical and you read what needs reading; the
person decides whether the tree moves, each edit you propose, each
verification, each sha256 value, each line they keep as it is, and whether
the follow-ups go in a commit or a pull request. Nothing is committed or
recorded before they said yes.

1. Run `docsys upgrade --json` and show the person the notes and the plan:
   `auto` items the tool applies, `manual` items that are theirs, `info`.
   Then `docsys upgrade --apply --commit`.
2. A CI workflow its owner edited (`ci-workflow`, at `file:line`): propose
   that line with the version the item names, and nothing else of theirs.
3. A pin listed for a re-read (`pins`): re-read it as the rules block says.
   If the page holds, run the item's `command`; if not, propose the page edit.
4. A verified page listed for a maintainer (`verified-record`): give the
   person its `docsys verify --show` line. Read the blocks with the person if
   they ask; the verification is theirs to record with the item's `command`.
5. A relay, skill, command or contract its owner edited (a diff): propose
   one text that keeps the owner's lines and takes the new ones.
6. A line of the team's own text that names a retired concept
   (`retired-concepts`, at `file:line`): propose that line rewritten with the
   item's replacement, keeping the owner's other words.
7. A sha256 value is never invented: ask the person, or point to the
   release page the workflow names.
8. The follow-ups are described by the upgrade commit's message
   (`git log -1 --format=%B`).
9. Last, the audit: the move ended with the leftover list — what an earlier
   version left, each with its file and its fix. Walk the person through each
   item, and through `docsys lint`, until both are clean, or what is left is
   what they keep.
