# Project file (`lexo.json`)

Lives in the project's data folder (`<translation folder>/.lexo/lexo.json`) and tells Lexo how the files of
that game are grouped for statistics. It is **optional**: without it the project has no groups and every file counts as
"unassigned". Groups are what the first game calls episodes, sections what it calls seasons.

```json
{
  "version": 1,
  "labels": { "section": "Season", "group": "Episode" },
  "sections": [
    { "n": 1, "status": "active",  "groups": [1, 2, 3, 4] },
    { "n": 2, "status": "active",  "groups": [5, 6, 7, 8] },
    { "n": 3, "status": "planned" }
  ]
}
```

Rules: `n` and group numbers are positive integers; a group belongs to one section only; `planned` sections are shown but
never counted. A broken file is ignored with a notice, never auto-repaired.

Not done yet: the UI still says "Epizoda" / "Sezóna" (the `labels` are stored but not used), the file is edited by hand,
and assigning files to groups still relies on the `ep1`-style name hints of the first game.

`language` (e.g. `"cs"`) selects the language pack for language-dependent checks (see `free/lexo/lang.py`); without it neutral rules apply, and legacy BaDIK data keeps `"cs"`.

## Groups without a hand-written file: Files > "Groups…"
A project that has no groups (a game other than Being a DIK) gets them from the file names: Lexo looks for numbers after `episode`/`ep`, `chapter`, `part`, `day`, `update`, `act`
(`ep1_cafe.rpy`, `scripts/chapter_03/…`), takes the word with the most different numbers and proposes `label` and `groups`; `season N` in the names makes sections when two or more seasons appear.
The dialog shows the proposal ("Episode 1-8, 44 of 72 files"), lets you change the word and the numbers, and writes `labels.group` and one section into `lexo.json` (other keys, such as `language`, stay).
The word is used everywhere ("Chapter 3" instead of "Episode 3"); the suggestion of assignments reads the numbers with the same word. A project with several sections (the BaDIK layout) is edited in the file by hand.
Until groups exist, files can only be "General" or unassigned, and the Overview offers "Set up groups…".

### Numbered series: `script`, `script2`, `script3`
Many games name their episodes by the main file: `script.rpy` (1), `script2.rpy`, `script3.rpy` … (A Wife and Mother: 4 files, Eternum: 9). Lexo proposes this when the numbers 1, 2, 3 … run without a gap
(at least three files, and in a project of more than 30 files at least 15 % of them; scene families such as `bedroom1, bedroom4, bedroom5` do not count), and the dialog has a tick box for it.
`lexo.json` then holds `"series": "script"`, and the suggestion of assignments maps `script` to group 1, `script2` to group 2 and so on (a number in a word like `ep2` is used as before).
Games that are organised by folder (Milfy City, LoSeSb) get no proposal; their groups would have to come from the folders (not built).
