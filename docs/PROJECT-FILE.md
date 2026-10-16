# Project file (`project.json`)

Lives in the project's data folder (`<translation folder>/_progress_data/project.json`) and tells Lexo how the files of
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

Migration: data written by the BaDIK tool (it has `prirazeni.csv` or `epizody.csv` but no `project.json`) keeps the
first game's layout (seasons 1-2, episodes 1-8), so nothing is dropped on opening. This rule goes away when the app can
write the file from its own UI.

Not done yet: the UI still says "Epizoda" / "Sezóna" (the `labels` are stored but not used), the file is edited by hand,
and assigning files to groups still relies on the `ep1`-style name hints of the first game.

`language` (e.g. `"cs"`) selects the language pack for language-dependent checks (see `free/lexo/lang.py`); without it neutral rules apply, and legacy BaDIK data keeps `"cs"`.
