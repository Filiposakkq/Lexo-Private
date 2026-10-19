# Tidying the repository

## What is untidy (found by an audit of the whole tree)

1. **The names of the folders say "Python".** `core/` and `free/` look like the product, but the product is `rust/`. They hold the Python reference implementation **and**
   files that the Rust product still uses (the UI, the language packs, the brand files, the golden data, the interface contract).
2. **The root `README.md` and the READMEs of `core/`, `free/`, `pro/` described the Python era.** (Fixed in this step: see below.)
3. **Two names for one thing:** the product is Aside, the code says Lexo. (Left on purpose; stated in the README.)
4. **Python files for a Python application nobody ships**: `free/Lexo.spec`, `build_exe.bat`, `run_manager.bat`, `run_lexo.py`, `requirements-build.txt`; they work and are harmless, but they look like the way to start the product.
5. **CI runs a Python suite (`free.yml`) whose main remaining value is guarding the UI texts** (every text has a Czech translation, no stale texts, the UI calls only existing operations).
6. The documents had no index and a mixture of current and historical content. (Fixed in this step: `docs/README.md`.)

## Done in this step (no code moved, nothing can break)

- New root `README.md`: what the product is made of (a picture), what every folder is (product / service / reference / test data), where to start, the commands.
- New `docs/README.md`: the index of the documents with their state.
- Banners in the READMEs of `core/`, `free/`, `pro/`: which part is the reference, which is the product.
- `HANDOFF.md` and `CLAUDE.md` point to the new map.
- `.gitignore`: the things that appear while working (`node_modules`, `.wrangler`, local settings, preview files).

## Stage 2: move the product files out of the Python folders (planned; needs a pull request of its own and CI)

Target layout:

```
app/            the product (today: rust/ + free/lexo_free/ui + core/lexo/packs + free/assets)
  rust/         (rust/ as it is)
  ui/           (free/lexo_free/ui)
  packs/        (core/lexo/packs)
  assets/       (free/assets)
pro/            private: rust/ (the pack), installer/, golden/ → testdata
server/  web/   services, unchanged
testdata/       golden/ (core/golden), contract/ (core/contract), pro-golden/ (pro/golden)
reference/python/   core/ free/ pro/lexo_pro pro/tests run_tests.py (the Python implementation, untouched inside)
docs/  .github/
```

What has to change when the files move (found by searching for the paths):

| Moved | Referenced by |
|---|---|
| `free/lexo_free/ui` | the three installer and Rust workflows, `lexo-app/tauri.conf.json`, `lexo-server` (default folder), `rust/tools/run_ui_selftest.py`, the Python tests of the UI texts |
| `core/lexo/packs` | the installer workflows, `tauri.conf.json`, the Rust tests of the pack, the Python Core (it loads the packs from inside its package: it needs a path setting) |
| `core/golden`, `core/contract`, `pro/golden` | `lexo-golden`, the contract test of `lexo-core`, the Pro tests, `.gitattributes`, `rust.yml`, the Python golden tools |
| `free/assets` | the icon and brand scripts, `tauri.conf.json` (icons) |

Order: (1) `git mv` in one commit so the history follows; (2) fix every path in one more commit; (3) run `cargo test` (core, server, pro), the golden run (749 of 749) and `vitest` locally; the Python suites and the installers can only be checked in CI, so the pull request is merged only when all of CI is green.

**Risk:** the Python suites cannot run on the owner's computer (no working Python there), so a wrong path in them shows up only in CI. That is why the move waits for a pull request that is allowed to take a few CI rounds.

## Stage 3: retire the Python implementation (decision of the owner)

What would be lost, and what it takes to keep it:

- the **reference** the port was checked against: not needed any more (the golden data are recorded files; the Rust code was also compared with Python on eight real games);
- the **generator of the golden data** (`core/tests/golden_tool.py`): needed only when a change of behaviour must be recorded; the Rust runner can be given an "update" mode first;
- the **guards of the UI texts** (`test_ui_i18n*`, `test_contract_ui`, `test_free_messages`): these must be ported (to Node or Rust) before Python is removed, otherwise nothing checks the Czech texts;
- the **CI minutes** of the Python jobs are saved (the Windows runners count double).

Safe way: tag the last commit with Python (`archive/python-reference`), port the guards, then delete `reference/python`, the Python workflows and the Python launchers.
