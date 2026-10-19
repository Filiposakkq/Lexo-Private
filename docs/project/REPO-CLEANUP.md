# Tidying the repository

## What was untidy (found by an audit of the whole tree)

1. **The names of the folders said "Python".** `core/` and `free/` looked like the product, but the product is `rust/`; they held the Python reference implementation **and** files that the Rust product still uses (the UI, the language packs, the brand files, the golden data, the interface contract).
2. The root README and the READMEs of the parts described the Python era; the documents had no index.
3. **Two names for one thing:** the product is Aside, the code says Lexo (left on purpose, stated in the README).
4. Python launchers for a Python application nobody ships (`Lexo.spec`, `build_exe.bat`, `run_manager.bat`, `run_lexo.py`) lay where they looked like the way to start the product.

## Done

**Step 1 (docs only):** the root `README.md` with a picture and a table of every folder, `docs/README.md` (the index of the documents and their state), banners in the READMEs of the parts, `HANDOFF.md` and `CLAUDE.md` pointing to the map, `.gitignore`.

**Step 2 (the move, no behaviour changed):** the product files left the Python folders, and the Python implementation went into one folder of its own:

| Was | Is |
|---|---|
| `free/lexo_free/ui` | `rust/resources/ui-classic` |
| `core/lexo/packs` | `rust/resources/packs` |
| `free/assets` | `rust/resources/assets` |
| `core/golden`, `core/contract`, `pro/golden` | `testdata/golden`, `testdata/contract`, `testdata/pro-golden` |
| `pro/lexo_pro/ui` (the Czech texts of the pack, read by the Rust pack) | `pro/rust/lexo-pro/ui` |
| `core/`, `free/` (the rest), `pro/lexo_pro`, `pro/tests`, `run_tests.py` | `reference/python/{core,free,pro}`, `reference/python/run_tests.py` |

The moves were made with `git mv` (the history follows). Every path was changed: the Rust crates (`lexo-server` defaults, `lexo-golden`, the tests, `tauri.conf.json`), the Pro pack, the workflows, the Python code of the reference (the folders it reads the UI and the packs from), the tools of `rust/tools`, `.gitattributes` and the documents. The recorded golden data were not touched.

**Checked:** the golden data 749 of 749, `cargo test` and clippy of Core, server and Pro pack, `cargo check` of the Tauri application (it reads `tauri.conf.json`), and the Python suites (Core 358, Free 222, Pro 68) in the new place. Not checked locally: the installers (they run in CI) and the UI self-test in a browser.

## Open: retire the Python implementation (a decision of the owner)

What would be lost, and what it takes to keep it:

- the **reference** the port was checked against: not needed any more (the golden data are recorded files; the Rust code was also compared with Python on eight real games);
- the **generator of the golden data** (`reference/python/core/tests/golden_tool.py`): needed only when a change of behaviour must be recorded; the Rust runner can be given an "update" mode first;
- the **guards of the UI texts** (`test_ui_i18n*`, `test_contract_ui`, `test_free_messages`): these must be ported (to Node or Rust) before Python is removed, otherwise nothing checks the Czech texts;
- the **CI minutes** of the Python jobs are saved (the Windows runners count double).

Safe way: tag the last commit with Python (`archive/python-reference`), port the guards, then delete `reference/python` and its workflow.
