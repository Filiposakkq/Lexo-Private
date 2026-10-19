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

## Built so far (version 1)
Team (`app_team.rs`, pack `team.rs`, UI `44-team.js`): joining with your name, the lead makes the team and gives roles and files, everybody sees the members and their files (column *Assigned to*, filter *Person*). Files written by one person only: `.aside/team/members/<author>.json` and `.aside/team/lead.json` (the lead). Pro only for making and managing the team (`team.manage`); the members work in Free. Also built: the activity log (`.aside/team/activity/<author>.jsonl`, what each person changed), **statistics per person** for the lead (Pro: progress of their files, changed, approved and marked lines, last activity), **notes to the team on lines** (`.aside/team/notes/`), and the **news** since your last look (Free, kept per computer). The full user guide is [TEAMS.md](../guide/TEAMS.md). Also built: leaving the team, removing and taking back people (a `removed` list in the lead's file, so a sharing tool cannot bring a removed person back), handing the team over (an offer that the other person accepts on their own computer, where the licence is checked, so the lead can never pass the team to someone who cannot manage it), ending it. Also built: the git buttons (**Get changes**, **Send changes**; Free). Not built: the merge of `.rpy` files by block (git merges text by lines itself; the block merge would only help sharing tools that make a copy of the file), notifications.

## Plan
After the Free release. Most Ren'Py translators work alone; teams with bigger projects are the ones who would pay for this.
