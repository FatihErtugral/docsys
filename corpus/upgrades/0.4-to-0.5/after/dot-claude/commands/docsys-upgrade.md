---
description: Move this repository's docs tree to the docsys you run — the tool applies what is mechanical, you read what needs reading, the person decides
allowed-tools: Bash(docsys *), Bash(git status:*), Bash(git diff:*), Bash(git log:*), Read, Edit
---

# /docsys-upgrade — move the tree, then finish what needs a person

Nothing is committed or recorded that the person did not say yes to.

1. Run `docsys upgrade --json` and show the person the notes and the plan:
   `auto` items the tool applies, `manual` items that are theirs, `info`.
   On their word: `docsys upgrade --apply --commit`.
2. A CI workflow its owner edited (`ci-workflow`, with a diff): propose one
   edit from the diff that keeps their `runs-on` and their install step.
   Apply it on their word.
3. A pin listed for a re-read (`pins`): re-read it as the rules block says.
   If the page holds, run the item's `command`; if not, propose the page edit.
4. A verified page listed for a maintainer (`verified-record`): give the
   person its `docsys verify --show` line. Read the blocks with the person if
   they ask; the verification is theirs to record with the item's `command`.
5. A relay, skill, command or contract its owner edited (a diff): propose
   one text that keeps the owner's lines and takes the new ones. Apply it on
   their word.
6. A line of the team's own text that names a retired concept
   (`retired-concepts`, at `file:line`): propose that line rewritten with the
   item's replacement, keeping the owner's other words. Apply it on their
   word; a line they keep stays as it is.
7. A sha256 value is never invented: ask the person, or point to the
   release page the workflow names.
8. The follow-ups are their own commit, or a pull request on the person's
   word, described by the upgrade commit's message (`git log -1 --format=%B`).
