# Roadmap

Rule for every step: the old test suite (or its port) must stay green, nothing is pushed or merged without the owner's approval,
and the game folders on disk are read-only (work on copies).

## Step 0 – Foundation (this repo)
Done when: `README.md`, `docs/`, empty `core/ free/ pro/`, the audit exists. *(started)*

## Step 1 – Generalise in Python
Copy the old package here (as `core/` + `free/` split comes in step 2), then:
1. Project file (`lexo.toml` or JSON in the project folder): language, shared files, groups (replacing seasons/episodes), source and target language, game folder.
2. Language packs: move Czech-specific rules out of `validator.py`, `spelling.py`, `memory.py`, `bulk.py` into `lang/cs`; add a neutral default.
3. Scene preview fallback values from the game's own `gui.rpy`, then Ren'Py defaults.
4. Remove BaDIK names, URLs, config folder; add a one-time migration for the author's installation.
5. UI strings into a table; English is the default, Czech is the second language.
6. Try on 3-4 other Ren'Py games (different structure, with and without `tl/`, with `.rpa`). Fix what breaks.
Done when: the same code opens Being a DIK and at least three other games, UI in English, 430 tests ported and green, plus new tests for the second language and for a game without groups.

## Step 2 – Split Core and UI
Core = no UI, no HTTP, no window. A documented interface (see ARCHITECTURE.md). Free is the first client.
Done when: Core has its own tests and can be driven from a script; Free contains only UI and wiring.

## Step 3 – New tools in Core
- Translation file generator (via installed SDK first).
- `.rpa` unpacking and `.rpyc` decompiling (unrpa, unrpyc) as optional helpers, with a clear message about licences of the helpers.
Done when: from a game folder without `tl/` the app creates the translation skeleton and opens it.

## Step 4 – Rust and a real desktop app
Tauri: Rust backend, web UI. Port Core module by module (`rpy` and `rpa` first, then lint, fontmetrics, project, store), each checked against the same test data as the Python version. The Python code stays as a reference until all modules match.
Done when: one exe without Python that passes the same fixtures.

## Step 5 – New UI
Designed after the Core interface is stable; menu with game selection, onboarding, settings.

## Step 6 – Free release and Pro
- Public repo from `core/` + `free/` (fresh history).
- Pro as a separate module downloaded after entering a licence; licence = signed key verified offline (Ed25519); Pro code stays private.
- Installer, updates, crash-safe project data.

## Open questions
- Final name and its availability (Lexo is a working choice).
- Licence of the public part and of Pro.
- Which features are Pro: proposal in docs/FEATURES.md (revision 2).

## Status
- Step 1.0 done: the Python app is imported into `free/` (package `lexo`, launcher `run_lexo.py`), renamed from BaDIK, 406 tests green (the 24 tests of the BaDIK website stayed in the old repo). Game-specific parts are still in place; see HARDCODED-AUDIT.md.
- Step 1.1 done: groups/sections come from `project.json` (docs/PROJECT-FILE.md); a project without it has no groups. 415 tests green.
- Step 1.2 done: language packs (`lexo/lang.py`, `language` in project.json); neutral rules by default, Czech as a pack. Free/Pro proposal in docs/FEATURES.md. 421 tests green.
- Step 2.0 done: feature registry in Core (`free/lexo/features.py`): catalog of gated features, free limits, gates in the app (mt batch, safe fixes, regex replace, bulk categorisation, publishing). Development default = everything on. Licensing decision (30-day check, 2 devices, private code) recorded in docs/FEATURES.md. 429 tests green.
