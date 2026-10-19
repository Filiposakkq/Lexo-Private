# Handoff: how work on Lexo is done (read this first in a new chat)

Written so that a new session (or a new person) knows what the project is, what has been decided, how a step is done, where the checkpoints are and what is open.
Keep it current: when a decision or a rule changes, change it here in the same commit.

## 1. What Lexo is
A translation manager for Ren'Py games: edit, proofread, check and track translations (`.rpy` files in `game/tl/<language>/`).
It grows out of the BaDIK Translation Manager (built for the Czech translation of *Being a DIK*; old repo `D:\RenPy Stuff\Being-A-DIK-CZ`, folder `tools/translation-manager`, which keeps working for that translation and received the same features by copying: menu tab, whole-word search, results bar, missed texts).
The product is called **Aside** (the repository, the crates and some old docs still say Lexo). Monorepo `Filiposakkq/Lexo-Private` (temporarily **public**: do not push premium Pro code until it is private again), local folder `D:\RenPy Stuff\Lexo`.

Goals, in order: (1) generalise from one game and one language to most Ren'Py games, English UI with Czech as the first language pack (**done**); (2) split Core and Free, put the Pro features in a separate pack, fix the interface and the golden data (**done**);
(3) **port Core to Rust (Tauri: Rust core, web UI)**, module by module against the golden data (**done up to M9**, M10 installer in progress); (4) only then the menu with game selection and the "prepare a game" window, a new UI, the licence server, the installer, the Free and Pro release.

## 2. Layout of the repository
The map with every folder and what kind it is (product, service, Python reference, test data) is in the root [README.md](../../README.md); the plan to tidy it is [REPO-CLEANUP.md](REPO-CLEANUP.md). The older tree below is the Python-era layout and is kept for the reference code.
```
reference/python/core/lexo/        package `lexo` = Core: project model, parser, writer, checks, App (app_*.py), the operations table (api.py), the extension host (extensions.py), language packs (packs/)
reference/python/core/tests/       Core tests + helpers.py, contract_tool.py, golden_tool.py
testdata/contract/    api.json, shapes.json (generated: what each operation takes and answers)
testdata/golden/      fixed inputs and the recorded answers for the Python / Rust comparison
reference/python/free/lexo_free/   package `lexo_free` = Free: server.py (local HTTP JSON server), cli.py (window, command line), ui/ (browser UI: ES modules ui/js/NN-name.js, entry 99-boot.js)
reference/python/free/tests/       Free tests (HTTP API, UI files, command line); reference/python/free/tests/ui = the UI self-test launchers
reference/python/pro/lexo_pro/     the Pro pack (private, never leaves this repository): publish, categorize, regex_replace, mt_batch, fix_safe, licence (limits)
reference/python/pro/tests/        the pack's tests
docs/             all documents; reference/python/run_tests.py runs the suites
```
The app is a local HTTP JSON server + a pywebview window (Edge/Chrome app window as a fallback). Core never imports Free or the pack; Free imports Core as `lexo`; the pack imports Core; Core finds the pack by name at run time.

## 3. Documents (what each one answers; the folders `guide/`, `product/`, `technical/`, `project/`, `history/` of `docs/`)
| File | Question it answers |
|---|---|
| README.md | The index of the documents and which ones are current |
| guide/USER-GUIDE.md | Everything the application can do, by tab |
| guide/PRO-FEATURES.md | What each Pro feature does, the demo limits, what it sends |
| technical/APP-ROUTES.md | What the application serves outside the contract of Core |
| project/REPO-CLEANUP.md | What was untidy in the repository and what was moved |
| product/PRIVACY.md | Everything that leaves the user's computer |
| project/ROADMAP.md | What are the steps, what is done, what is next (the table "Where we are" and the open decisions are kept current; the log is the history) |
| technical/ARCHITECTURE.md | Layers, where things are, the operations table and the contract, how Pro attaches |
| technical/API.md | The interface of Core: every operation (generated) |
| technical/GOLDEN.md | The golden data: inputs, answers, normalisation, how a port is checked |
| technical/RUST-PORT.md | The plan of the port to Rust: layout, modules, milestones M0 to M10 with "done when", tests, risks, the Pro connection; the Progress table is kept current |
| product/PRO-DESIGN.md | How Pro is kept apart, delivered, licensed, updated; limits and demo; the licence server proposal; what is done and open |
| product/FEATURES.md | Free and Pro features with their status; coupling; licensing options |
| technical/GAME-TOOLS.md | `.rpa` extraction, unrpyc helper, generating translation files, scene preview, layered images, choices and their menu |
| technical/UI-TEXTS.md | How texts are translated (`t`, `msg`, packs), tests that guard it |
| technical/DATA-LAYOUT.md | Data layout (`.aside/`, local data, events) |
| technical/PROJECT-FILE.md | `.aside/aside.json`: groups, language |
| product/TEAMWORK.md | How several translators would work together |
| history/HARDCODED-AUDIT.md | What was tied to one game or one language and what is left on purpose |
| history/GAME-TESTS.md | The eight games the app was tried on and what they showed |

## 4. Decisions already made (do not re-open without the owner)
- **English text is the key** for every user-visible string. UI: `t("...")`, `tn`, `tx`, `labels`, `N_`. Python: `msg("... {x}", x=...)`, `msgn`, `join`. Czech lives only in `rust/resources/packs/cs/ui.json` (keep it sorted), in the pack rules and in `pro/rust/lexo-pro/ui/cs.json` (texts of messages written in the pack). Never wrap a string at module load time (guarded by a test).
- **Language packs are data** (`pack.json`, `rules.json`, `ui.json`); the app must work without any pack.
- **Pro** (details in PRO-DESIGN.md): Pro code is never in Core or Free; a Pro feature is a JSON call to the pack (`ExtensionHost.call`); without the pack it is locked and no flag can change that; limits and the demo live in the pack (`licence.py`); taste features ship as a demo pack inside the installer, premium features are downloaded after a licence and are encrypted per licence and watermarked; 2 devices per licence; online check every 30 days (14 for suspicious ones) with a grace period; **one-off payment** per major version. All five gated features are in the pack.
- **Data layout** (DATA-LAYOUT.md): shared small files in `<translation folder>/.aside/`, local/bulky data in `%APPDATA%\Lexo\projects\<id>\`. Ren'Py 7.4.10 does not exclude nested dot folders from a build, so nothing private or bulky goes into the game folder.
- **unrpyc / unrpa** are only external helpers (unrpyc is MIT: keep the notice if bundled; unrpa is GPL and is not used), native `.rpa` extraction is built in, translation files are generated by the Ren'Py SDK.
- **Interface**: Core is used through the operations table (`reference/python/core/lexo/api.py`); Free's server has no operation of its own. The contract files and the golden answers are generated and tested; a change of them is a decision.
- **No menu / new UI / "prepare a game" window / licence server / release before the Rust port.** (The preparation of a game works in code and is tested on 8 games, but only from a script.)
- There is no migration from the older BaDIK tool (one user, who keeps using it); nothing in the code reads the Czech file names or the `_progress_data` folder.
- The layered images of a game (`layeredimage`, composites) are not drawn in the scene preview; the preview says so. Planned for later.

## 5. Safety rules (hard)
1. The game folders on disk are **read-only**. Work on copies or synthetic projects. Never write into a real game or into the BaDIK translation (the game scripts under `testdata/golden/projects` are synthetic).
2. Never push, open a PR, merge, publish or delete anything outside the working tree **without asking the owner first** (the owner answers "Push + PR"). Describe in the PR what is **not verified**. Never force-push (the tool blocks it); open a new branch instead.
3. Never commit secrets, licence keys or private keys. Pro code does not go into `core/` or `free/` (a test checks it).
4. Tests must stay green; do not weaken or delete a test to get green. A flaky test is investigated, not skipped.
5. Writes to user data are atomic and preceded by a backup; the app never deletes a folder it does not own.
6. Do not change files outside `D:\RenPy Stuff\Lexo` unless the owner asked (the old repo is a separate project; when the owner asks for a feature there, work on its own branch and PR).

## 6. How a step is done (the loop)
1. **Plan**: read ROADMAP.md and the relevant doc; write the step down (what, why, what "done" means) before coding. If a design choice is the owner's, ask; otherwise choose the conventional option and note it.
2. **Branch** from fresh `main`: `feat/<short-name>` (or `fix/`, `docs/`, `refactor/`). One topic per branch.
3. **Implement in small commits.** After each commit run the tests (section 7). Use scripts for mechanical bulk edits and prove them lossless; for multi-line text write a file with the Write tool, not a shell heredoc (backslashes get lost).
4. **Checkpoints** (all must hold before asking to push):
   - `py reference/python/run_tests.py` green (Core, Free, Pro).
   - UI self-test green when anything in `ui/` or the server changed: with the pack (default in the monorepo) and, when a Pro step changed, also without it (`LEXO_NO_PRO=1`).
   - After a change of an operation: `py reference/python/core/tests/contract_tool.py`; after a change of behaviour that the golden data records: `py reference/python/core/tests/golden_tool.py`, and read the diff of what changed.
   - English UI smoke test (`?lang=en`, all tabs) when texts changed.
   - New behaviour has new tests; a bug fix has a test that failed before.
   - Docs updated in the same branch: ROADMAP.md (log + table), the specific doc, this file if a rule changed.
   - `git status` clean, no scratch files in the repo, no servers left running.
5. **Ask** the owner: "Push + PR?" Then push, open the PR into `main` with: what changed, how verified, **what is not verified**, and the Claude Code line at the end. PRs of one topic are stacked when they depend on each other (base = the earlier branch).
6. CI (`.github/workflows/free.yml`: `reference/python/run_tests.py` + UI self-test in Edge via Playwright) runs on the PR. The self-test is occasionally flaky on slow runners; look at the log, do not just rerun blindly. The owner merges; then `git checkout main && git pull` and start the next step.

## 7. Commands (Windows; from the repository root `D:\RenPy Stuff\Lexo` unless said)
```
py reference/python/run_tests.py                                  # all suites: core, free, pro (pro only where reference/python/pro/tests exists); `core`, `free` or `pro` runs one; more arguments go to unittest
py reference/python/core/tests/contract_tool.py                   # regenerate testdata/contract/*.json and docs/technical/API.md after a change in reference/python/core/lexo/api.py
py reference/python/core/tests/golden_tool.py                     # regenerate testdata/golden/expected after a change of behaviour that is meant
py reference/python/free/run_lexo.py [folder]                     # the app (the pack is on the path when pro/ exists; LEXO_NO_PRO=1 = as Free; LEXO_PRO_DEMO=1 = the demo limits)
cd free; py tests/ui/serve_selftest.py --no-open # synthetic project + a URL; open it with ?selftest=1&lang=cs; free\run_ui_selftest.bat opens the browser
```
Stop self-test servers when done (`python ... serve_selftest.py` processes). `?lang=en` / `?lang=cs` switches the UI language. Set `PYTHONIOENCODING=utf-8` for scripts that print Czech.
The shell is Git Bash / PowerShell. Tests of the core suite run without Free on the import path (`reference/python/run_tests.py` sets the paths); running `unittest discover` by hand needs `PYTHONPATH=core;reference/python/core/tests`.

## 8. Where things are
- `App` (`reference/python/core/lexo/app.py`) is assembled from parts by area: `app_core.py` (open/refresh/settings, the extension host, `_gate`, `_ext_call`), `app_validation.py`, `app_entries.py`, `app_glossary.py`, `app_proof.py`, `app_assign.py`, `app_memory.py`, `app_game.py`, `app_snapshots.py`, `app_lint.py`, `app_fixes.py`, `app_mt.py`, `app_publish.py`, `app_missed.py`, `app_menu.py`; exceptions and constants in `app_base.py`. A test keeps every part under 450 lines. New methods go to the part of their area, never back into `app.py`.
- Operations: `reference/python/core/lexo/api.py` (80 operations: name, method, effect, typed parameters, example). Server dispatch in `reference/python/free/lexo_free/server.py` (`api.ROUTES`); the session routes (windows, focus, folder dialog, pings) stay there.
- Pro: `reference/python/core/lexo/extensions.py` (host), `reference/python/core/lexo/features.py` (catalog of the five gated features), `reference/python/pro/lexo_pro/__init__.py` (provider), `reference/python/pro/lexo_pro/licence.py` (limits, the development stub of the licence).
- Messages: `reference/python/core/lexo/messages.py`; wire format `{"$m","p","$n"}` / `{"$j"}`; browser side `ui/js/05-i18n.js` (`fromWire`), `ui/js/10-api.js`.
- Project layout: `layout.py` (reads `.aside/aside.json`), languages: `lang.py`, game tools: `gametools.py`, `rpa.py`, scene preview: `gameimages.py`, choices: `menus.py`, missed texts: `missed.py`.
- Tests that guard conventions: `test_ui_i18n.py`, `test_ui_i18n_load_time.py`, `test_python_messages.py`, `test_free_messages.py`, `test_messages.py`, `test_pro_boundary.py` (no Pro code in the public packages), `test_contract.py`, `test_contract_ui.py`, `test_golden.py`, `test_app_parts.py`.

## 9. Current state and what is next
**Update (read this first).** The Rust port is done to M9 (749/749 golden, 8 968 read calls identical on 8 real games); the Tauri app, the NSIS installer and a Pro installer are built in CI. The Pro pack is Rust (`pro/rust/lexo-pro`, a separate private workspace; sidecar program `lexo-pro-sidecar`) with six gated features (the five old ones and `carry.over`), the signed licence file, the activation client and demo limits. The licence server is `server/` (Cloudflare Worker + D1 + Resend, admin page). Python is still the reference but no longer shipped. Rust commands (MSVC toolchain only, Windows): set `RUSTUP_HOME=E:/rust/rustup CARGO_HOME=E:/rust/cargo CARGO_TARGET_DIR=D:/rust-target-msvc` and use `cargo +stable-x86_64-pc-windows-msvc`; golden: `LEXO_NO_PRO=1 cargo run -q --locked -p lexo-golden` in `rust/`; pack tests: `cargo test` in `pro/rust`. Python tests run only in CI on this machine. Standing rules: the private licence key lives outside the repository (`E:\AsideKeys`), secrets are never typed into chat, the pack is built with `--no-default-features` for a release (`dev-unlock` is a development default). Open: payment provider, web and legal pages, lost-key flow, expiry dates, signing, updater, more Pro features (fuzzy memory, style check, release builder; see FEATURES.md). The paragraphs below describe the state before the port and are kept for the reasons in them.

Steps 1 to 6 of the pre-port list are done (ROADMAP.md): English source texts and Czech as a pack; the `.aside/` data layout and event log; generalisation tried on 8 games; the split into Core, Free and Pro with the five gated features in the pack; the interface table, the contract and the golden data.
**Next: the port to Rust (Tauri), planned in RUST-PORT.md (start with M0).** Module by module, each checked against `testdata/golden` (see GOLDEN.md): `rpy` and `rpa` first (they need only their own golden cases), then `lang`, `validator`, `layout`, `menus`, `fontmetrics`, `project`, `store`, `events`, and the operations on top, until the whole `scenarios` pass. The Python Core stays as the reference until all of them match. **The port covers Free Core only**: the golden data has no Pro operation; the Pro pack (Python today) is not ported in this step and is decided with the sidecar transport (PRO-DESIGN.md), which can keep it a separate program in any language. In the meantime the Rust Core must answer the five Pro features as "locked" and call the pack through the same JSON host.
Open decisions of the owner (details in ROADMAP.md): the licence and openness of Core and Free before the first release (open source such as GPL, or closed freeware with an EULA; Pro stays closed either way), the name "Lexo" (check it is free), the price, whether the old BaDIK tool is replaced by Lexo for the real translation work and when.
After the port: the menu with game selection and onboarding, the "prepare a game" window, the licence server (PRO-DESIGN.md), signed packs and updates, the installer, the user guide, layered images in the preview.

## 10. Working with the owner
- The owner writes Czech, fast and informally; answer in Czech, code and docs in English.
- The owner often says "Je to na tobě": decide the next step yourself, record the decision, keep going; ask only for things in section 5 and for decisions that are the owner's (licence, price, naming, what is Pro).
- Report honestly: failing tests, skipped steps and unverified parts are stated plainly. Say plainly when something is a limit, not a bug, and what the owner would see.
- Keep answers short; the details belong in the docs and the PR description.
