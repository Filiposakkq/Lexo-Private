# Golden data: fixed inputs and the answers Core gives to them

Roadmap step 6. The port of Core to Rust (and any other implementation) has to behave like this Python Core, **byte for byte where files are written**. This folder holds what it is
compared against: small fixed inputs, and the recorded answers to them. Nothing here depends on Python.

```
testdata/golden/
  rpy/cases/*.rpy, rpy/cases.json     translation files for the parser (and the language pack to parse with: cs or neutral)
  rpa/archives/*.rpa                  small .rpa archives (version 2, version 3, with a prefix, a hostile index, garbage, truncated)
  layout/cases.json                   file name lists for the group proposals, paths for the folder and series numbers
  menus/scripts/*.rpy, menus/cases.json   game scripts with menus and the calls (script, line, choice) to the menu finder
  validator/cases.json                source / target pairs for the checks
  spelling/cases.json                 a vocabulary (translated texts), words asked for a hint, checks with that vocabulary
  textedit/cases.json                 find / replace cases in the plain, whole-word and stem modes (protected parts, errors), `case_like`, `mask`
  fonts/*.ttf, fonts/not_a_font.bin   tiny fonts made by `reference/python/core/tests/make_golden_fonts.py` (cmap formats 4 and 12, bad files) for the advance widths
  data/<name>.json                    data scenarios: files, a fixed clock and steps that call the functions of the data modules (see below)
  projects/<name>/game/...            whole projects (a game with scripts and game/tl/czech): basic, episodes, folders, messy
  scenarios/<name>.json               a project and a list of operation calls ({op, params}) run in order on a copy of it
  expected/...                        the recorded answers (generated; JSON, normalised)
```

## Scope: Free Core only, not the Pro pack
The golden data covers what Core does **without the Pro pack**: the 73 operations that need no Pro feature (of the 80 operations, seven belong to the five gated features: `publish/plan`, `publish/confirm`, `mt/targets`, `mt/batch`, `fix/preview`, `fix/apply`, `assign/suggest`, and the regex mode of `replace/*`; without the pack they answer "locked" and the scenarios do not call them).
The Pro pack has its own tests (`reference/python/pro/tests`, Python) and is **not part of the Rust port of Core**. How the pack is ported or kept is decided together with the sidecar transport (docs/product/PRO-DESIGN.md): because it is called through plain JSON
(feature, method, payload), it can stay a separate program written in any language, and golden data for it (the requests and answers of each Pro method) can be added the same way when that decision is made.

## What a scenario is
Open `game/tl/czech` of the project (a copy in a temporary folder), then run every step with `api.call(app, op, params)` (docs/technical/API.md) and record the answer (or the error message).
At the end record every `.rpy` of the translation folder: its SHA-1 and its text (or base64 when it is not UTF-8). So reads, writes (`entry/save`, `assign`, `groups/apply` ...) and
their undo are all compared, and a port that writes a line differently by one byte fails.

## What a data scenario is
`data/<name>.json` holds `files` (path under a temporary folder -> text, or `{"base64": ...}`), a `clock` (`local` and `utc` time, fixed) and `steps`: `{"op": "store.assign", ...parameters}`.
The runner writes the files, runs the steps in order with the clock replaced, records the answer (or the error text) of every step and, at the end, every file under the folder
(SHA-1 and text): so the CSV files of the store, the event logs, `history.csv`, `config.json`, the backups and the zips are compared as files. The ops: `csv.read`, `csv.text`,
`write_csv`, `atomic_write`, `make_backup`, `config.*`, `datadir.*`, `lock.*`, `layout.use`, `store.*`, `events.*`, `history.*`, `texthistory.*`, `memory.*`, `snapshots.*`, `project.*`, `fixes.*`, `export.*`, `bulk.*`, `gameimages.*`, `missed.*`, `lint.*` (the Python and SDK programs are fakes),
`editor.*` (finders and starters of programs are fakes; `<TMP>` stands for the temporary folder), `gametools.*` (unrpyc and the Ren'Py generator are fake programs that answer from a list and write the files they are told to; the archives come from `testdata/golden/gametools`, made by `make_archives.py`), `mt.*` (the network is a fake that answers from a list and records the requests), plus `advance` / `set_epoch` / `set_local` (the clock), `write_file`, `remove_file`, `list_dir`. Both runners implement the same ops (`DataRunner` in `golden_tool.py`, `data.rs` in `lexo-golden`).

Things the data scenarios settle on purpose:
- **The clock.** `datetime.now()` is the fixed local time, `datetime.now(utc)` the fixed UTC time (so the names of backups, the stamps of events and the manifests of snapshots are known); `time.time()` is the real
  time shifted by `advance` steps (files have real times; the rule "is the newest backup old enough" needs a shift), or fixed by `set_epoch` (the age of a lock). `set_local` moves the local time.
- **Random by design** (a lock token and time, a project id): `lock.json` and `project-id` are not recorded; the answers only say that an id was made and is kept.
- **Zips.** The bytes of a zip depend on the compressor, so a `.zip` is recorded as the list of its files with their SHA-1, and the `size` of a snapshot is left out of the answers.
- **`text_history.jsonl`** is written by Python in text mode (CRLF on Windows): it is recorded with LF.
- **Order.** Python's `approved` data is a set; `store.flatten` sorts it now so the events are written in the same order every time. Snapshots with the same time are listed in the order of their names.

## Normalisation (what is not compared)
Answers are plain JSON with messages as their English text. Before comparing: the temporary folder becomes `<TMP>` (with `/` as the separator), the random project id in a data folder
path becomes `<id>`, snapshot and backup names lose their time stamp, values of keys that are times or random (`time`, `date`, `updated`, `elapsed`, `token`, `loaded_at`, `seconds`, `author` ...)
become `<volatile>`, and a path to a translation file in a folder is written with `/` (a scenario also writes them with `/`; the runner turns them into the separator of the system for the call).
Two runs in different folders at different times give identical files; a test checks it.

## For the Rust port
Implement the small runner in the port's own tests: read the inputs, call the port's operations with the same names and parameters, normalise, compare with `expected/`. Port module by module
(roadmap step 4 of the old numbering: `rpy` and `rpa` first, then `lint`, `fontmetrics`, `project`, `store`): the parser and archive cases need only those modules, the scenarios need the whole of Core.
Where Python's own wording leaks into an answer (pickle or zlib messages) the golden data keeps only the part Core writes itself.

## Python side
- `py reference/python/core/tests/golden_tool.py` regenerates `expected/` from the inputs. Look at the diff before committing it: a changed answer is a changed behaviour.
- `reference/python/core/tests/test_golden.py` checks that Core still gives the recorded answers, that every input has an answer and no answer is stale, and that nothing belonging to one machine is recorded.
- `.gitattributes` marks `testdata/golden/**` as not text, so a checkout never changes a line end (the digests are those of the bytes).
- New cases: add the input file, run the tool, commit both. The Pro features are not part of the golden data (see "Scope" above).
