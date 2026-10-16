# Audit: what in the old tool is tied to one game or to Czech

Source: `Being-A-DIK-CZ/tools/translation-manager/prehled` (v1.7.0, ~7 400 lines of Python, 430 tests).
Found with a text search plus reading the key modules; the list is a starting point, not a proof of completeness.
Status of every row is "to do" until a step in the roadmap handles it.

## 1. Game structure (Being a DIK only)
| Where | What is hard-wired | Generalisation |
|---|---|---|
| ~~`seasons.py`~~ -> `layout.py` (done), `app.py` UI texts, `bulk.py` name hints | Layout comes from `project.json` (see PROJECT-FILE.md). Still open: UI wording and the `ep1` file-name hints. |
| `bulk.py` `SHARED_STEMS` | File names `header, options, screens, gui, common, title_screens, fonts_czech` are treated as shared | Detect shared files by rule (no dialogue) and let the project file override. |
| `config.py` `skip_files` | `common.rpy`, `fonts_czech.rpy` | Per-project setting, empty by default. |
| `export.py`, `publish.py`, `web` config | Progress JSON for the BaDIK website, git repo and web path | Becomes an optional "publish progress" feature (candidate for Pro or a plug-in). |
| `app.py` `INFO` (repo, web URLs) | `Filiposakkq/Being-A-DIK-CZ`, `badik.filiposakkq.dev` | Product constants in one place. |
| `gameimages.py` | Scene images from `.rpa` or `game/images` – mostly generic; image naming rules were tuned on BaDIK | Test on other games; make the lookup rules configurable. |
| `app.py` `GUI_DEFAULTS` | 1920x1080, name box position and text size of BaDIK as fallback for the scene preview | Fallback = Ren'Py defaults (1280x720 or the game's own `gui.rpy`), not BaDIK values. |

## 2. Language (Czech only)
| Where | What | Generalisation |
|---|---|---|
| `rpy.py` | Docstrings and parsing assume `translate czech <id>`; check that the language name is a variable, not a literal | Language = project setting (`game/tl/<language>`). |
| `validator.py` | Czech word lists, doubled words, number-words ("14" -> "čtrnáct"), English function words that are not Czech | Language packs: a `lang/<code>.py` with its own rules. Core keeps only language-neutral checks (tags, variables, brackets, length, box fit). |
| `spelling.py` | Czech spell checking, pairs like byt/být | Plug-in per language; off when no dictionary exists. |
| `memory.py`, `bulk.py` (`interjections`) | Czech notes on declension and interjections | Move the wording into language packs. |
| `mt.py` | `source_lang` "EN", `target_lang` "CS" defaults | Defaults come from the project. |
| `fontmetrics.py` | Workaround for patched fonts with added Czech letters (wrong `nominalWidthX`) | Stays valid, but is not Czech-specific; rename the comments. |
| `app.py` glossary | `{English: Czech}` naming in code and UI | Source and target term. |

## 3. Product name and storage
| Where | What |
|---|---|
| `config.py` | `%APPDATA%\BaDIKTranslationManager`, `badik-translation-manager`, `PREHLED_CONFIG_DIR` -> `Lexo` and `LEXO_CONFIG_DIR`; a migration from the old folder for the author's own installation |
| `cli.py`, `server.py`, `__init__.py`, spec/bat files | Window titles, class names, exe name |
| package name `prehled` (Czech for "overview") | Rename to the real package name |

## 4. UI language
- `ui/app.js` has ~690 lines containing Czech characters, `index.html` ~12. UI texts are mixed into the code, so they must first be moved to a string table (`en` default, `cs` as the second language), only then translated.
- Server messages in Python (`app.py`, `validator.py`, `project.py` labels such as "Nezačato", "Hotovo") are Czech too; they must return keys, not sentences.

## 5. Things that are fine as they are
Atomic writes and the one-writer rule (`fixes.py`), the incremental project refresh, snapshots, text history, lock file, the line parser structure, the `.rpa` reader and the font width logic are not tied to the game.

## 6. New features planned for after the generalisation
- Extracting scripts from `.rpyc` (unrpyc), unpacking `.rpa` archives (unrpa) – both already exist in the Python world; on disk: `D:\RenPy Stuff\unrpyc-2.0.4`. In Rust these are separate modules and have to be ported or called as helpers.
- Generating translation files like the Ren'Py SDK does (`renpy.sh <game> translate <language>`): first by calling an installed SDK (one of them is at `D:\RenPy Stuff\renpy-7.4.10-sdk`), later with an own generator.
