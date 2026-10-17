# Golden data: fixed inputs and the answers Core gives to them

Roadmap step 6. The port of Core to Rust (and any other implementation) has to behave like this Python Core, **byte for byte where files are written**. This folder holds what it is
compared against: small fixed inputs, and the recorded answers to them. Nothing here depends on Python.

```
core/golden/
  rpy/cases/*.rpy, rpy/cases.json     translation files for the parser (and the language pack to parse with: cs or neutral)
  rpa/archives/*.rpa                  small .rpa archives (version 2, version 3, with a prefix, a hostile index, garbage, truncated)
  layout/cases.json                   file name lists for the group proposals, paths for the folder and series numbers
  menus/scripts/*.rpy, menus/cases.json   game scripts with menus and the calls (script, line, choice) to the menu finder
  validator/cases.json                source / target pairs for the checks
  projects/<name>/game/...            whole projects (a game with scripts and game/tl/czech): basic, episodes, folders, messy
  scenarios/<name>.json               a project and a list of operation calls ({op, params}) run in order on a copy of it
  expected/...                        the recorded answers (generated; JSON, normalised)
```

## What a scenario is
Open `game/tl/czech` of the project (a copy in a temporary folder), then run every step with `api.call(app, op, params)` (docs/API.md) and record the answer (or the error message).
At the end record every `.rpy` of the translation folder: its SHA-1 and its text (or base64 when it is not UTF-8). So reads, writes (`entry/save`, `assign`, `groups/apply` ...) and
their undo are all compared, and a port that writes a line differently by one byte fails.

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
- `py core/tests/golden_tool.py` regenerates `expected/` from the inputs. Look at the diff before committing it: a changed answer is a changed behaviour.
- `core/tests/test_golden.py` checks that Core still gives the recorded answers, that every input has an answer and no answer is stale, and that nothing belonging to one machine is recorded.
- `.gitattributes` marks `core/golden/**` as not text, so a checkout never changes a line end (the digests are those of the bytes).
- New cases: add the input file, run the tool, commit both. The Pro features are not part of the golden data (a Free Core is what is ported first); their answers are checked in `pro/tests`.
