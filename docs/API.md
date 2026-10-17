# The interface of Core (generated)

Generated from `core/lexo/api.py` by `py core/tests/contract_tool.py`; do not edit by hand (a test fails when it is out of date).

Every operation is a JSON call: parameters in, a dict with `"ok": true` out. Over HTTP (Free) a GET takes its parameters in the query string, a POST in the JSON body;
a script or the Rust port calls the operation by name. Errors come back as `{"ok": false, "error": ...}`. Messages in answers are `{"$m": template, "p": values}`
(see `docs/UI-TEXTS.md`). Effects: **read** changes nothing, **write** changes the project or its data folder, **external** also reaches outside
(a program, a window, the network). `*` marks a required parameter. The recorded shapes of the answers are in `core/contract/shapes.json`.

80 operations.

| Operation | Method / path | Effect | Parameters | What it does |
|---|---|---|---|---|
| `approve/apply` | POST `/api/approve/apply` | write | `selections` (list), `on` (flag, default true) | Approve (or un-approve) lines. |
| `approve/preview` | POST `/api/approve/preview` | read | `file`* (str), `line`* (int), `scope` (str, default "project"), `same_speaker` (flag, default true) | Preview approving identical lines. |
| `approve/rules` | POST `/api/approve/rules` | read | `rules` (any), `files` (list) | Preview approving by rules (names, interjections). |
| `assign` | POST `/api/assign` | write | `name`* (str), `v` (str) | Assign a file to a group. |
| `assign/apply` | POST `/api/assign/apply` | write | `items` (list) | Apply assignments. |
| `assign/suggest` | POST `/api/assign/suggest` | read | `names` (list) | Suggested group for files (a Pro feature: the Pro pack suggests, the app shows the groups to choose from). |
| `consistency` | GET `/api/consistency` | read | `loose` (flag), `by_speaker` (flag) | Originals translated in several different ways. |
| `consistency/unify` | POST `/api/consistency/unify` | write | `key` (str), `target` (str), `lines` (any), `loose` (flag), `by_speaker` (flag) | Make all translations of one original the same. |
| `entries` | GET `/api/entries` | read | `file` (str), `status` (str, default "all"), `kind` (str), `q` (str), `issues` (str), `sort` (str, default "line"), `offset` (int), `limit` (int, default 200), `around` (int), `unit` (str), `word` (flag) | A page of translation lines, filtered and sorted on the server. |
| `entry` | GET `/api/entry` | read | `file`* (str), `line`* (int) | The detail of one line (texts, state, findings, proofreading). |
| `entry/check` | POST `/api/entry/check` | read | `file`* (str), `line`* (int), `text` (str) | The check of a text the translator is typing (nothing is written). |
| `entry/context` | GET `/api/entry/context` | read | `file` (str), `line` (int) | The original game script around the line this translation comes from. |
| `entry/history` | GET `/api/entry/history` | read | `file` (str), `line` (int) | The versions of a line's text over time. |
| `entry/memory` | GET `/api/entry/memory` | read | `file` (str), `line` (int) | Other translations of the same original (translation memory). |
| `entry/menu` | GET `/api/entry/menu` | read | `file` (str), `line` (int) | The menu a line belongs to: question, choices with translations, what follows each. |
| `entry/mt` | POST `/api/entry/mt` | external | `file`* (str), `line`* (int) | A machine translation suggestion for one line (uses the network). |
| `entry/next` | GET `/api/entry/next` | read | `file` (str), `line` (int), `kind` (str) | The next line of a kind (todo, review, fix, over) after this one, wrapping round the file. |
| `entry/save` | POST `/api/entry/save` | write | `file`* (str), `line`* (int), `text` (str) | Write a new translation of one line into the .rpy file (backup, atomic, undoable). |
| `entry/scene` | GET `/api/entry/scene` | read | `file` (str), `line` (int), `radius` (int, default 6) | The lines around one line (original and translation). |
| `episode` | POST `/api/episode` | write | `ep`* (int), `state`* (str) | Set the state of a group (episode). |
| `export/preview` | GET `/api/export/preview` | read | - | The public progress data and its checks (writes only the local progress.json). |
| `features` | POST `/api/features` | read | - | Which gated (Pro) features are available, with which limit, and which Pro pack is installed. |
| `file/save` | POST `/api/file/save` | write | `name`* (str), `c` (str), `t` (str) | Save the counts of a file (the manual correction columns). |
| `files` | GET `/api/files` | read | - | The translation files with their counts and states. |
| `fix/apply` | POST `/api/fix/apply` | write | `items` (list), `file` (str), `kinds` (list), `selections` (any) | Apply the chosen fixes. |
| `fix/last` | GET `/api/fix/last` | read | - | What the last bulk fix changed (for undo). |
| `fix/preview` | POST `/api/fix/preview` | read | `items` (list), `file` (str), `kinds` (list) | Preview the bulk fixes (speaker, spaces, typos ...). |
| `fix/undo` | POST `/api/fix/undo` | write | - | Undo the last bulk fix. |
| `game/characters` | GET `/api/game/characters` | read | - | The characters defined in the game scripts: code, name, colour. |
| `game/font` | GET `/api/game/font` | read **binary** | `k` (str) | A font of the game by file name (never a path): (bytes, mime). |
| `game/fontmetrics` | GET `/api/game/fontmetrics` | read | `k` (str) | Advance widths of a font of the game (key = file name). |
| `game/image` | GET `/api/game/image` | read **binary** | `g` (str), `k` (str) | A picture of the game by the key an answer gave (never a path): (bytes, mime). |
| `game/scene` | GET `/api/game/scene` | read | `file` (str), `line` (int) | The picture(s) behind a line and the text box metrics for the scene preview. |
| `glossary` | GET `/api/glossary` | read | - | The glossary: speakers and terms with counts and examples. |
| `glossary/delete` | POST `/api/glossary/delete` | write | `codes` (list) | Delete glossary items. |
| `glossary/import-apply` | POST `/api/glossary/import-apply` | write | `items` (list) | Import character names. |
| `glossary/import-preview` | POST `/api/glossary/import-preview` | read | `path` (str) | Preview importing character names from a game file. |
| `glossary/occurrences` | GET `/api/glossary/occurrences` | read | `code` (str), `missing` (flag) | Where a speaker code (or a missing name) occurs. |
| `glossary/set` | POST `/api/glossary/set` | write | `items` (list) | Set glossary items. |
| `groups/apply` | POST `/api/groups/apply` | write | `label` (any), `groups` (any), `series` (str), `folders` (any) | Define the groups of the project (written to lexo.json). |
| `groups/suggest` | GET `/api/groups/suggest` | read | - | The groups the project has now and the groups its file names suggest. |
| `ignore/list` | GET `/api/ignore/list` | read | - | The findings the translator told the app to ignore. |
| `ignore/set` | POST `/api/ignore/set` | write | `items` (list), `on` (flag, default true), `note` (str) | Ignore (or stop ignoring) findings on lines. |
| `lint/start` | POST `/api/lint/start` | external | - | Start the Ren'Py lint of the whole game (needs the SDK). |
| `lint/status` | GET `/api/lint/status` | read | - | State and result of the Ren'Py lint run. |
| `missed` | GET `/api/missed` | read | `level` (str, default "likely") | Texts the Ren'Py generator missed, found in the game scripts. |
| `missed/add` | POST `/api/missed/add` | write | `items` (any), `into_files` (flag) | Add texts the generator missed to the translation files. |
| `missed/functions` | GET `/api/missed/functions` | read | - | Functions of the game that are called again and again with a text. |
| `missed/ignore` | POST `/api/missed/ignore` | write | `texts` (any), `clear` (flag) | Leave texts out of the missed-texts list for good. |
| `missed/settings` | POST `/api/missed/settings` | write | `functions` (any) | Set the project's own functions that show text. |
| `mt/apply` | POST `/api/mt/apply` | write | `selections` (any) | Write the chosen machine translations. |
| `mt/batch` | POST `/api/mt/batch` | external | `items` (any) | Machine translate many lines (uses the network). |
| `mt/targets` | POST `/api/mt/targets` | read | `file` (str), `lines` (any), `only_todo` (flag, default true) | The lines a batch machine translation would take. |
| `open-editor` | POST `/api/open-editor` | external | `file`* (str), `line` (int, default 1) | Open a file at a line in the editor. |
| `open-url` | POST `/api/open-url` | external | `id` (str) | Open one of the known addresses in the browser (by id). (needs no open project) |
| `overview` | GET `/api/overview` | read | - | Totals, groups, estimates and the data of the Overview tab. |
| `prefs` | POST `/api/prefs` | write | - | Remember UI preferences (the body is the object of preferences). (needs no open project) |
| `project/set` | POST `/api/project/set` | write | `path` (str), `force` (flag) | Open a translation folder (and remember it). (needs no open project) |
| `proof/clear-file` | POST `/api/proof/clear-file` | write | `file`* (str) | Clear the proofreading marks of a file. |
| `proof/next` | GET `/api/proof/next` | read | `file` (str), `after` (int) | The next line still to proofread after a line number. |
| `proof/set` | POST `/api/proof/set` | write | `items` (list), `state` (str, default "ok"), `note` (any) | Mark lines as proofread / to fix, with a note. |
| `proof/upto` | POST `/api/proof/upto` | write | `file`* (str), `line`* (int), `state` (str, default "ok") | Mark everything up to a line as proofread. |
| `publish/confirm` | POST `/api/publish/confirm` | external | `token` (str), `branch` (str) | Publish the progress data after the typed confirmation (a Pro feature). |
| `publish/plan` | POST `/api/publish/plan` | read | - | Preview publishing the progress data (a Pro feature). |
| `refresh` | POST `/api/refresh` | write | `force` (flag), `dirty` (flag), `discard` (flag) | Read the files again (merging what changed on disk). |
| `replace/apply` | POST `/api/replace/apply` | write | `selections` (list), `find` (any), `replace` (any), `mode` (str, default "plain"), `case` (flag), `files` (list), `protect` (flag, default true) | Apply a find and replace to the chosen places. |
| `replace/preview` | POST `/api/replace/preview` | read | `find` (any), `replace` (any), `mode` (str, default "plain"), `case` (flag), `files` (list), `protect` (flag, default true) | Preview a find and replace over the translations. |
| `scenes` | GET `/api/scenes` | read | `file` (str), `line` (int) | The scenes of a file (runs of lines after the same `scene` statement) with progress. |
| `settings` | GET `/api/settings` | read | - | The settings (validation, text box, machine translation, publishing ...). |
| `settings/update` | POST `/api/settings` | write | - | Change settings (the body is the patch). (needs no open project) |
| `snapshots` | GET `/api/snapshots` | read | - | The snapshots (zips of the .rpy files) of the project. |
| `snapshots/create` | POST `/api/snapshots/create` | write | - | Make a snapshot of the .rpy files now. |
| `snapshots/delete` | POST `/api/snapshots/delete` | write | `name` (str) | Delete a snapshot. |
| `snapshots/files` | GET `/api/snapshots/files` | read | `name` (str) | The files inside one snapshot. |
| `snapshots/restore` | POST `/api/snapshots/restore` | write | `name` (str), `files` (any) | Restore files from a snapshot. |
| `state` | GET `/api/state` | read | - | What the client needs at start: is a project open, notices, config error, UI preferences, version. (needs no open project) |
| `ui-lang` | GET `/api/ui-lang` | read | `code` (str) | The interface texts of one language: {code, name, plural, strings}; English is an empty table. (needs no open project) |
| `ui-langs` | GET `/api/ui-langs` | read | - | The interface languages (language packs that have UI texts). (needs no open project) |
| `undo` | POST `/api/undo` | write | - | Undo the last change (approval, proofreading, glossary). |
| `validation` | GET `/api/validation` | read | `severity` (str), `category` (str), `file` (str), `q` (str), `offset` (int), `limit` (int, default 200), `ignored` (str, default "hide"), `group` (str), `word` (flag) | The findings of the translation check, one item per line. |
