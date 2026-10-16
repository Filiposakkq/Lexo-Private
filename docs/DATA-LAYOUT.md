# Data layout (target design; parts 2a and 2b built)

**Built (step 2a):** the files in `_progress_data` have English names, English column headers and English values, and the Czech names are read only by
`free/lexo/migrate.py`, which copies them once when a project is opened (old files stay untouched, nothing is overwritten):

| Old (BaDIK tool) | New | Columns |
|---|---|---|
| korekce.csv | proofread.csv | file;proofread;translated_manual |
| prirazeni.csv | assignments.csv | file;group (`general`, `episode N`) |
| schvaleno.csv | approved.csv | file;block;original |
| epizody.csv | episode_states.csv | episode;state (`none wip tr proof done`) |
| korektura_radky.csv | line_marks.csv | file;block;original;state;hash;note;date |
| ignorovano.csv | ignored.csv | file;block;original;code;hash;note;date |
| slovnik.csv | glossary.csv | code;name;type;note |
| historie.csv | history.csv | date;translated;total;percent;proofread;proofread_percent |
| historie_textu.jsonl | text_history.jsonl | (JSON lines, unchanged) |

The BaDIK layout is still detected from the old marker files (`prirazeni.csv`, `epizody.csv`), so a migrated legacy project keeps its groups.
**Built (step 2b):** the shared folder is `<translation folder>/.lexo/` (constant `DATA_DIR_NAME` in `project.py`, the only place to change if the product gets another name) with the CSV files above, `lexo.json` (was `project.json`) and `project-id`.
Local data lives in `<config folder>/projects/<project id>/` (on Windows `%APPDATA%/Lexo/projects/<id>`): `backups/`, `snapshots/`, `lock.json`, `text_history.jsonl`, `last_fix.json`, and the derived `progress.csv` / `progress.json`.
`free/lexo/datadir.py` creates the id, finds the local folder and, the first time a project is opened, **copies** (never moves or deletes) the old `_progress_data`: shared files to `.lexo/` (then `migrate.py` renames them), backups, snapshots and the text history to the local folder. Existing destination files are never overwritten.
**Not built yet (2c):** the per-author event log.
The rest of this document is the target design.

Where Lexo writes, and why. Today (imported BaDIK tool) the data is in `<translation folder>/_progress_data/` as seven Czech-named CSV files.
This is the target; the move is a separate step with a migration (see the end).

## Rules
1. Lexo writes only inside the translation folder (`game/tl/<language>/`, which is itself inside `game/`) and never anywhere else in the game: the `.rpy` files of the translation in place (after a backup, atomic write) and its own folder `.lexo/`.
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
    (moved out of the game folder, see above:)
    cache/               composed state, statistics, exported progress.json (disposable)
    texthistory.jsonl    older wordings of lines, for undo (git already versions the .rpy)
    backups/             copies of files before every write
    snapshots/           zip backups of the whole translation folder
    lock.json            one running copy per project
```
The `local/` block above lives under `%APPDATA%\Lexo\projects\<project id>\`; the project id is stored in `lexo.json`.
**Hidden folders are NOT reliably left out of a Ren'Py build.** Checked in the 7.4.10 SDK (`renpy/common/00build.rpy`): the default
patterns exclude only entries starting with a dot at the root (`.*`), then classify everything else (`**`) into the game, so
`game/tl/czech/.lexo/` can end up in a built game. Consequences:
- everything bulky or private stays out of the game folder: `local/` (cache, backups, snapshots, text history) is moved to `%APPDATA%\Lexo\projects\<project id>\` and `.lexo/` keeps only the small shared files;
- the shared part may still reach a build, so it contains no secrets and no personal data beyond the author name the user chose; the app shows the one-line rule to add to `options.rpy` (`build.classify("**/.lexo/**", None)`) and does not write it itself;
- the same holds for the old `_progress_data`.

## Events
One JSON object per line: `{"v":1, "t":"2026-10-17T09:12:00Z", "a":"<author id>", "k":"proofread", "line":"<block id>", "h":"<text hash>", ...}`.
Kinds: translated (manual override), proofread / flagged / unflagged, approved, ignored, assigned (file -> group), group state, glossary change.
- State = replay of all authors' files; a cached composed state in `local/cache/` makes opening fast and is rebuilt when the files change.
- Undo is a new event, never a deleted line. Progress history and "who did what" are computed from the events.
- **Identity of a line.** Ren'Py builds the `translate` id from the label plus the first 8 hex digits of an md5 of the block's code (checked in
  `renpy/translation/__init__.py`), unless the dialogue has an explicit `id`. A changed original therefore gets a **new id**, and the same id
  can never connect an old translation to the new line. Events record: the block id, a hash of the source text, and also the **label, the
  position inside the label and the speaker**. A proofread mark on an old line stays valid only while id and hash match (as today: otherwise stale).
- **Carrying a translation over to a new game version (Pro)** is a fuzzy match on label, position, speaker and similarity of the original text;
  the carried translation is marked "check", and a `proofread` event of the old line becomes "check" on the new one, never "checked".

## Other places
- `%APPDATA%\Lexo\config.json`: settings of the app and the list of known projects (no secrets). Error log next to it.
- `%APPDATA%\Lexo\packs\`: downloaded language packs. `%APPDATA%\Lexo\memory\`: translation memory shared by several games (Pro).

## Migration from the old layout
On first opening a project that has `_progress_data` and no `.lexo`: read the old CSV files, write the new files, leave the old folder untouched, and tell the user.
The old Czech file names are read only by the migration code.

## Open questions
- Exact event fields and compaction of very old events (a local compaction only, never rewriting shared files).
- Whether `glossary.json` should also be event-sourced (probably not: small, edited rarely).
