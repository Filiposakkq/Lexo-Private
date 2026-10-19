# Team work (Pro, planned)

## Version 1: no server (shared folder or git)
The team shares the translation folder with git, or a sync tool (Dropbox, Syncthing). Lexo needs no server of its own.

- **State** (proofread, approved, assigned...) lives in `.aside/events/<author>.jsonl`; every author writes only their own file, so merging never conflicts.
- **Translations in `.rpy` files** can be edited by two people in one file. Lexo merges **per `translate` block** (the block id is unique), not per text line, so different lines of one file do not collide.
- **A real conflict** (both changed one line) is shown, and the user picks a version or edits by hand.
- **Assignments:** the lead gives files or groups to people so that they do not overlap; this prevents most conflicts for free.
- **Roles:** translator and proofreader. The proofreader sees translated lines, and the proofread mark is written under their name. Statistics show who translated and checked what, computed from the events.
- **Not in version 1:** live co-editing in the same moment, notifications. People pull and push (or wait for the sync). Because git is awkward for non-technical translators, Lexo wraps it in buttons ("get changes", "send changes").

## Version 2: server (only if there is demand)
A hosted service for synchronisation, live assignments and notifications. It is a separate product with operation, accounts, security and cost, so it is not built ahead of time.

## What is built early, even without teams
Built (roadmap 2c): the per-author event log for the project state (`events.py`, docs/technical/DATA-LAYOUT.md). Not built: merge of `.rpy` files per block, roles, statistics from events, the git buttons.

The per-author event log and the stable line identity (block id + text hash) (see DATA-LAYOUT.md). Team work is then an addition, and the same identity serves "carry a translation over to a new game version".

## Plan
After the Free release. Most Ren'Py translators work alone; teams with bigger projects are the ones who would pay for this.
