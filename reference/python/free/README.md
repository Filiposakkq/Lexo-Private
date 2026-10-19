> **The Python application (`lexo_free/*.py`, `run_lexo.py`, the `.bat` files, `Lexo.spec`) is the old, unshipped version.** The product is the Tauri application in [`../rust/`](../../../rust/). What is still used by the product here: the **user interface** `lexo_free/ui/` (shared by the Rust server and the Tauri app) and `assets/` (icons, brand). Map of the repository: [`../README.md`](../../../README.md).

# Lexo (the application)

A translation manager for Ren'Py games: open the translation of any Ren'Py game, edit it, proofread it, check it and follow the progress.
*Lexo is a working name.* This repository is **private** and the application is under development (see `../../../docs/project/ROADMAP.md` for the plan and
`../../../docs/project/HANDOFF.md` for how work on it is done). It has been tried on eight Ren'Py games of different kinds (Ren'Py 7.4 and 8, with and without
`.rpa` archives, layered images, folders or numbered scripts); the results and the bugs found are in `../../../docs/history/GAME-TESTS.md`.

The application is Free, the package `lexo_free` (`reference/python/free/lexo_free/`, on top of the Core package `lexo` in `reference/python/core/lexo`): a local HTTP server with a JSON interface, a browser UI (ES modules in `lexo_free/ui/js/`) shown in an
own window (pywebview, Edge WebView2) or in an Edge/Chrome app window. Only the Python standard library is needed to run it.

## Start
```
py run_lexo.py                         opens the app (from free/) (it remembers the last project)
py run_lexo.py "D:\games\MyGame\game\tl\czech"
run_manager.bat [folder] [options]     the same from any folder (Python 3.9+; pywebview is optional)
```
Options: `--nogui` prints the summary and writes the project files, `--todo` (with it) lists only unfinished files, `--publish` shows the publishing
preview and asks for confirmation, `--find=text` searches. The UI is English; Czech is the first language pack (`?lang=cs`, or Settings).

**What to open:** the folder of one translation, `game/tl/<language>`. If you choose the game folder (or its `game` or `tl` folder) and it holds exactly
one translation, that one is opened; with several, the app asks which.

## Where the data lives
- `<translation folder>/.lexo/`: small shared text files (CSV, JSON): proofreading marks, assignments, glossary, `lexo.json` (groups, language), `missed.json`,
  `events/<author>.jsonl` (every change, one file per author, for working in a team through git or a shared folder).
- `%APPDATA%\Lexo\projects\<id>\`: local data (backups, snapshots, the project lock, text history, derived progress files).
- `%APPDATA%\Lexo\config.json`: settings. Nothing bulky or private is written into the game folder. The `.rpy` files of the translation are changed only by explicit
  actions, after a backup and with an atomic write. Details: `../../../docs/technical/DATA-LAYOUT.md`.

## What it does (Free)
- **Files and lines:** list, filters, search, editor with tags and variables protected, undo, snapshots of the translation folder.
- **Proofreading:** per-line marks, quick translate and quick proofread modes, focus mode, a second window.
- **Checks:** tags, variables, brackets, length, dialogue-box fit, spelling hints, glossary terms (language-dependent rules come from the language pack).
- **Find and replace, safe fixes, approving identical lines, translation memory and consistency checks.**
- **Scene preview** with the game's pictures and fonts, over the dialogue box of the game. It is an estimate from the game script. Layered images are not supported
  yet (planned): the preview then shows the background with a note, or no picture.
- **Groups and progress:** episodes, chapters, parts, days, updates, acts, `script`/`script2` series or one group per folder (proposed from the file names, `../../../docs/technical/PROJECT-FILE.md`);
  statistics per file and per group, completion estimate, weekly goals.
- **Preparing a game that has no translation yet** (no UI yet, `reference/python/core/lexo/gametools.py`): unpack scripts from `.rpa`, decompile with the external helper unrpyc, generate the
  translation files with the game's own engine. See `../../../docs/technical/GAME-TOOLS.md`.
- **Texts the generator missed** (experimental): `show text`, input prompts, titles in variables; found in the game's scripts and added to `extra_strings.rpy`.
- **Optional:** Ren'Py lint through the game's engine or an SDK, machine translation of a line (own API key, experimental), publishing progress to a website.
Which features will be Free and which Pro: `../../../docs/product/FEATURES.md`.

## Tests
```
py ../run_tests.py                            the Python tests (Core, then Free; `core` or `free` for one of them)
py tests/ui/serve_selftest.py                    the UI self-test: starts the app on a synthetic project and opens the browser (title SELFTEST PASS (n))
run_ui_selftest.bat                              the same
```
CI (`../../../.github/workflows/free.yml`) runs both on pull requests. Conventions that the tests guard: every user-visible text is English in the code and translated by a
language pack (`../../../docs/technical/UI-TEXTS.md`); no module may call `T()` without importing it; no part of the application class grows past 450 lines.

## Build the exe
```
build_exe.bat            runs the tests, then builds
build_exe.bat /notest
```
or `py -m pip install -r requirements-build.txt` then `py -m PyInstaller --noconfirm --clean Lexo.spec`. Result: `dist\Lexo.exe` (one file, no console). Building is a manual step;
nothing is released or published automatically.

## Limitations
- Windows is the platform used so far; the code avoids Windows-only calls where it can, but nothing else has been tried.
- Layered images, the frames of the text box and name box in the preview, and the exact fonts are approximations.
- Games with encrypted or modified archives, or with scripts that unrpyc cannot read, are not covered.
- Only one language pack (Czech) exists; the neutral rules apply to other languages.
