# Data layout (target design; parts 2a, 2b and 2c built)

**Built (step 2a):** the data files have English names, English column headers and English values:

| File | Columns |
|---|---|
| proofread.csv | file;proofread;translated_manual |
| assignments.csv | file;group (`general`, `episode N`) |
| approved.csv | file;block;original |
| episode_states.csv | episode;state (`none wip tr proof done`) |
| line_marks.csv | file;block;original;state;hash;note;date |
| ignored.csv | file;block;original;code;hash;note;date |
| glossary.csv | code;name;type;note |
| history.csv | date;translated;total;percent;proofread;proofread_percent |
| text_history.jsonl | (JSON lines) |

There is no migration from the Czech-named files of the older BaDIK tool any more: it had one user, and that user keeps working with that tool.
**Built (step 2b):** the shared folder is `<translation folder>/.aside/` (constant `DATA_DIR_NAME` in `project.py`, the only place to change if the product gets another name) with the CSV files above, `aside.json` (was `project.json`) and `project-id`.
Local data lives in `<config folder>/projects/<project id>/` (on Windows `%APPDATA%/Lexo/projects/<id>`): `backups/`, `snapshots/`, `lock.json`, `text_history.jsonl`, `last_fix.json`, and the derived `progress.csv` / `progress.json`.
`reference/python/core/lexo/datadir.py` creates the id and finds the local folder.
**Built (step 2c):** the event log (`reference/python/core/lexo/events.py`, `Store.sync_events`):
- `.aside/events/<author>.jsonl`, one file per author, one JSON object per line: `{"v":1,"t":<UTC time>,"a":<author>,"kind":<kind>,"key":[...],"value":<json or null>}`. Kinds: `corr`, `assign`, `approved`, `episodes`, `proof`, `ignored` (the glossary stays a small file). `value: null` means removed; undo is a new event.
- The author id (`<user name>-<4 hex>`) is made on first use and kept in the settings (`author`).
- The state is the replay of all files; for one key the newest event wins (time, then author). The CSV files stay as the cache that is read at start and may still be edited in Excel: at start-up and at every refresh, differences between the memory and the log become events of this author, and events written by others since the last read are merged in and written to the CSV files.
- Every computer remembers how much of each file it has read in `<local>/events-seen.json`. If that note is lost while events exist, the log wins over old CSV files; with no events yet, the current state is written as the first events.
- Clocks decide conflicts, so two people changing the same key within seconds of clock difference may overwrite each other (the later one stays). Compaction of old events and showing conflicts are not built.
The rest of this document is the target design.

Where Lexo writes, and why.

## Rules
1. Lexo writes only inside the translation folder (`game/tl/<language>/`, which is itself inside `game/`) and never anywhere else in the game: the `.rpy` files of the translation in place (after a backup, atomic write) and its own folder `.aside/`.
2. Shared project data (what a team needs) is separated from local and derived data (what nobody shares).
3. Text formats only (JSON, JSONL), versioned, so they can be read by eye and merged in git. No binary database for shared data.
4. Anything that is derived can be deleted and is rebuilt from the shared data.
5. A feature never deletes or rewrites a folder it does not own (Free keeps `pro/` untouched, also after a licence runs out).

## Folder
```
<translation folder>/.aside/
  aside.json              project settings: language, groups (sections), SDK, labels      [shared]
  glossary.json          glossary: names of characters and terms                          [shared]
  events/<author>.jsonl  every change of state, append-only, one file per author          [shared]
  pro/                   data of Pro features (character profiles, project memory)        [shared]
  team/                  members/, lead.json, activity/, notes/ (one file per author; `lead.json` has `removed` and `offer`; see docs/guide/TEAMS.md) [shared]
  .gitignore             excludes local/
  local/                                                                                  [local only]
    (moved out of the game folder, see above:)
    cache/               composed state, statistics, exported progress.json (disposable)
    texthistory.jsonl    older wordings of lines, for undo (git already versions the .rpy)
    backups/             copies of files before every write
    snapshots/           zip backups of the whole translation folder
    lock.json            one running copy per project
```
The `local/` block above lives under `%APPDATA%\Lexo\projects\<project id>\`; the project id is stored in `aside.json`.
**Hidden folders are NOT reliably left out of a Ren'Py build.** Checked in the 7.4.10 SDK (`renpy/common/00build.rpy`): the default
patterns exclude only entries starting with a dot at the root (`.*`), then classify everything else (`**`) into the game, so
`game/tl/czech/.aside/` can end up in a built game. Consequences:
- everything bulky or private stays out of the game folder: `local/` (cache, backups, snapshots, text history) is moved to `%APPDATA%\Lexo\projects\<project id>\` and `.aside/` keeps only the small shared files;
- the shared part may still reach a build, so it contains no secrets and no personal data beyond the author name the user chose; the app shows the one-line rule to add to `options.rpy` (`build.classify("**/.aside/**", None)`) and does not write it itself;

## Events (built, see above)
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

## Open questions
- Exact event fields and compaction of very old events (a local compaction only, never rewriting shared files).
- Whether `glossary.json` should also be event-sourced (probably not: small, edited rarely).

## The old names

Before the application was called Aside, the shared folder was `.lexo/` and its layout file `lexo.json`. When a project is opened, a folder with the old names is renamed once to `.aside/` and `aside.json` (`datadir.adopt_legacy`, in both cores); nothing is changed when the new folder exists already. Both names are skipped when files are scanned, so a leftover `.lexo/` is never taken for part of the game.
