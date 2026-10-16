# Data layout (target design, not built yet)

Where Lexo writes, and why. Today (imported BaDIK tool) the data is in `<translation folder>/_progress_data/` as seven Czech-named CSV files.
This is the target; the move is a separate step with a migration (see the end).

## Rules
1. Lexo never writes into the game, only into the translation folder (`game/tl/<language>/`): `.rpy` files in place (after a backup, atomic write) and its own folder `.lexo/`.
2. Shared project data (what a team needs) is separated from local and derived data (what nobody shares).
3. Text formats only (JSON, JSONL), versioned, so they can be read by eye and merged in git. No binary database for shared data.
4. Anything that is derived can be deleted and is rebuilt from the shared data.
5. A feature never deletes or rewrites a folder it does not own (Free keeps `pro/` untouched, also after a licence runs out).

## Folder
```
<translation folder>/.lexo/
  lexo.json              project settings: language, groups (sections), SDK, labels      [shared]
  glossary.json          glossary: names of characters and terms                          [shared]
  events/<author>.jsonl  every change of state, append-only, one file per author          [shared]
  pro/                   data of Pro features (character profiles, team, project memory)  [shared]
  .gitignore             excludes local/
  local/                                                                                  [local only]
    cache/               composed state, statistics, exported progress.json (disposable)
    texthistory.jsonl    older wordings of lines, for undo (git already versions the .rpy)
    backups/             copies of files before every write
    snapshots/           zip backups of the whole translation folder
    lock.json            one running copy per project
```
The dot in the name matters: Ren'Py leaves hidden folders out of a build by default (verify on 7.x and 8.x). The old `_progress_data` is not hidden.

## Events
One JSON object per line: `{"v":1, "t":"2026-10-17T09:12:00Z", "a":"<author id>", "k":"proofread", "line":"<block id>", "h":"<text hash>", ...}`.
Kinds: translated (manual override), proofread / flagged / unflagged, approved, ignored, assigned (file -> group), group state, glossary change.
- State = replay of all authors' files; a cached composed state in `local/cache/` makes opening fast and is rebuilt when the files change.
- Undo is a new event, never a deleted line. Progress history and "who did what" are computed from the events.
- A line is identified by the Ren'Py block id plus a hash of the source text, so a changed original makes an old proofread mark stale (as today) and the same identity can be used to carry translations over to a new game version (Pro).

## Other places
- `%APPDATA%\Lexo\config.json`: settings of the app and the list of known projects (no secrets). Error log next to it.
- `%APPDATA%\Lexo\packs\`: downloaded language packs. `%APPDATA%\Lexo\memory\`: translation memory shared by several games (Pro).

## Migration from the old layout
On first opening a project that has `_progress_data` and no `.lexo`: read the old CSV files, write the new files, leave the old folder untouched, and tell the user.
The old Czech file names are read only by the migration code.

## Open questions
- Exact event fields and compaction of very old events (a local compaction only, never rewriting shared files).
- Whether `glossary.json` should also be event-sourced (probably not: small, edited rarely).
