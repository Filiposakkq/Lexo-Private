# Port of Core to Rust: plan

Roadmap step 7. This document is the plan; the table "Progress" at the end is kept current (one row per milestone, updated in the same commit as the work).
Scope: **Free Core only** (what `core/golden` covers). The Pro pack is not ported in this step; how it connects afterwards is in section 9 and in PRO-DESIGN.md.

## 1. Goal and principles
- One executable without Python: a Rust Core inside a Tauri application, with the existing browser UI. Behaviour **identical** to the Python Core, checked by data (`core/golden`, docs/GOLDEN.md), not by reading code.
- **No feature work during the port.** A port that also changes behaviour cannot be checked. Bugs found in the Python Core are fixed in Python first (golden answers regenerated, diff read), then ported.
- **Python stays the reference** until every milestone passes; it is not deleted in this step.
- **Small, verifiable milestones.** Each one: (a) golden cases for the module exist (added on the Python side first when missing), (b) the Rust module passes them, (c) the documents are updated. One PR per milestone, stacked when they depend on each other.
- The interface does not change: the 80 operations (docs/API.md) keep their names, parameters and JSON answers. The UI changes in one place only: how a call reaches Core (section 8).

## 2. Repository layout
```
rust/                       Cargo workspace (private monorepo, next to core/ free/ pro/)
  Cargo.toml
  crates/
    lexo-core/              the library: modules mirror core/lexo (rpy, rpa, lang, layout, validator, ...); no UI, no HTTP; file access only through std::fs
    lexo-golden/            the golden runner (a test and a command): reads core/golden, calls lexo-core, normalises, compares with expected/
    lexo-server/            (milestone 7) a small local HTTP server with the same routes as Free's, for the UI self-test and development
  app/                      (milestone 8) the Tauri application: one command `call(op, params)`, the session routes (windows, focus, folder dialog)
```
Rust names follow the Python modules (`rpy.rs` for `rpy.py`) so that a reviewer can put the two side by side. Public API of a module = what the Python module exports and the operations use.

## 3. Prerequisites
**Installed so far (with the owner's permission): the Rust toolchain 1.99.1 on drive E:**, because C: has only about 9.7 GB free and the owner did not want it on C: or D:. It is the GNU host (`x86_64-pc-windows-gnu`), profile minimal, in `E:\rust` (`rustup`, `cargo`, `target`); nothing is added to PATH or changed in the system, so each command sets `RUSTUP_HOME=E:\rust\rustup`, `CARGO_HOME=E:\rust\cargo` and puts `E:\rust\cargo\bin` first in `PATH`. `rustup-init.exe` was checked against the published SHA256 before it ran. A hello world builds. This is enough for `lexo-core` and the golden runner (M0 to M7); **MSVC Build Tools are not installed**, and whether Tauri (M8) is built with MSVC or with the GNU host is decided at M8 (CI has MSVC anyway). The list below is what the port still needs:
- Rust toolchain (stable, via rustup) and the **MSVC Build Tools** on Windows; for Tauri also WebView2 (present with Edge). **Installing them is a download: ask the owner first.** CI installs them itself (`windows-latest` has MSVC and a Rust toolchain).
- Crates (checked for licence before use: MIT / Apache-2.0 only, no GPL): `serde`, `serde_json`, `fancy-regex` (Python's patterns use look-around, which the plain `regex` crate does not have: 8 lines in 5 modules), `flate2` (zlib), `zip` (snapshots), `sha1`, `csv`, `unicode-segmentation` / `unicode-normalization` if a golden case shows a difference, `ureq` (machine translation over HTTPS), `tauri` 2.x, `tiny_http` or `axum` (the dev server), `tempfile` (tests). A small **restricted pickle reader** for `.rpa` indexes is written by hand (only dict, list, tuple, int, str, bytes; any global is refused, as `rpa.py` does) rather than pulled in as a dependency.
- A CI job `rust` (`.github/workflows/rust.yml`): `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` on `windows-latest` (and `ubuntu-latest` as soon as the golden runner is green, to find path and encoding assumptions early).

## 4. Module inventory
Sizes are the lines of the Python module (a Rust module is usually 1.3 to 1.8 times as long). "Golden" says what already checks it; "to add" = golden cases written (in Python) as the first task of its milestone.
| Module | Lines | Depends on | Golden today | Golden to add | Milestone |
|---|---|---|---|---|---|
| `util` (norm_ws, atomic write, CSV, backups) | 143 | - | scenarios | CSV and atomic-write cases | M0 / M3 |
| `messages` (msg, wire, render) | 101 | - | answers carry the English text | wire format cases | M1 |
| `lang` (packs, plural rules, letters) | 228 | messages | parser cases (cs, neutral) | plural rules, pack loading, `ui-lang` | M1 |
| `rpa` (archive reader) | 73 | messages | `rpa.json` | - | M1 |
| `rpy` (parser, classify, count) | 264 | lang, util | `rpy.json` | - | M1 |
| `layout` (groups) | 291 | lang | `layout.json` | `lexo.json` parse and save | M2 |
| `menus` | 168 | - | `menus.json` | - | M2 |
| `validator` | 453 | rpy, lang, util | `validator.json` | spelling, terms, text box | M2 |
| `spelling`, `textedit`, `fontmetrics` | 96, 81, 101 | util | scenarios (partly) | spelling hints, replacer modes, font advance widths | M2 |
| `store`, `events`, `history`, `texthistory`, `memory` | 434, 164, 258, 114, 118 | util, rpy | scenarios | the data files in and out (CSV, JSONL), event merge, history | M3 |
| `snapshots`, `lock`, `config`, `datadir` | 203, 98, 121, - | util | scenarios (snapshots) | zip contents, lock file, config file | M3 |
| `project`, `export`, `bulk` | 328, 223, 205 | rpy, store, layout | scenarios | refresh and diff of files on disk, export JSON | M4 |
| `fixes` (the single writer) | 273 | rpy, util | scenarios (files after writes) | CRLF / BOM / failed write and restore | M4 |
| `gameimages`, `missed`, `gametools`, `lint`, `editor`, `mt` | 342, 299, 355, 131, 159, 182 | rpa, rpy, util | scenarios (partly) | image index, missed texts, SDK calls (a fake SDK), a fake HTTP | M5 |
| `app_*` and `api` (the 73 operations of Free) | ~2 400, 568 | all | 4 scenarios, 93 calls | more scenarios as the operations get covered | M6 |
| `server` (HTTP) | 420 | api | free tests | - | M7 |
Not ported in this step: `extensions` is ported (the host is Core), the Pro pack is not (section 9).

## 5. Milestones
Each milestone ends with: its golden cases pass (`cargo test`), the CI job is green, docs and the Progress table are updated, and the owner has merged the PR. Order follows dependencies; a milestone does not start before the one it needs is merged.

**M0 Foundations.** Workspace, CI job, the golden runner skeleton in `lexo-golden` (reads `core/golden`, the same normalisation as `golden_tool.py`, prints a readable diff), `util::norm_ws` as the first function with a test. Rust toolchain installed on the owner's machine (asked). *Done when:* `cargo test` runs in CI and the runner reports "0 of N" with a clear list.

**M1 Parser and archives.** `messages` (the subset: `msg`, `wire`, `render`), `lang` (packs, letters, plural rules), `rpa` (index with the restricted pickle reader, refusal of a hostile index), `rpy` (parser, `classify`, `count`). *Done when:* `rpy.json` (13 cases, Czech and neutral pack) and `rpa.json` (6 archives) match exactly, including the digests and the English texts of the structure problems. First real risk: the pickle subset and Unicode letter classes.

**M2 Checks and layout.** `layout` (propose, folder and series numbers, `lexo.json`), `menus`, `validator` (all codes), `spelling`, `textedit` (plain, word, stem modes), `fontmetrics`. *Done when:* `layout.json`, `menus.json`, `validator.json` match and the new golden cases for spelling, replacer modes and font widths do.

**M3 Data files.** `store` (assignments, corrections, approvals, proofreading, glossary, ignores, undo), `events` (per-author log, last writer wins), `history`, `texthistory`, `memory`, `snapshots` (zip), `lock`, `config`, `datadir`. *Done when:* the new data-file cases (input files, the state read from them, the bytes written back) match, including the merge of two authors' event logs.

**M4 Project and the writer.** `project` (files, refresh, entries, counts, status), `export`, `bulk` (approving identical lines, rules), `fixes` (the **single writer**: backup, atomic write, verify, restore, CRLF and BOM). *Done when:* the read operations of `basic`, `episodes`, `folders`, `messy` match and `files_after` of the write steps is byte-identical (the first scenario steps that write).

**M5 Game tools.** `gameimages` (the index of pictures, layered recognition, the stack of shown pictures), `missed`, `gametools` (workspace, unrpyc and SDK as external programs), `lint`, `editor`, `mt` (HTTP). External programs and the network are replaced by fakes in the golden cases. *Done when:* `entry/context`, `entry/scene`, `game/scene`, `scenes`, `entry/menu`, `game/characters`, `missed` answer as recorded.

**M6 The operations.** The 73 Free operations of the table on top of the modules, `call(app, op, params)` with the same coercion of parameters (docs/API.md, `Param.coerce`), the locked answers of the five Pro features, the extension host (interface 1, the plain-JSON rule). *Done when:* all four scenarios (93 calls and the files after them) match, and `api.json` equals the Rust table (a test compares them).

**M7 Dev server and the UI against the Rust Core.** `lexo-server` serves the UI files and the same routes as Free's server; the UI self-test (Playwright) runs unchanged against it. *Done when:* the UI self-test is 139/139 against the Rust server.

**M8 The Tauri application.** One command `call(op, params)`; the session routes (windows, focus between windows, folder dialog, fullscreen) as commands and events; `ui/js/10-api.js` calls the command instead of `fetch`; the window with the UI. *Done when:* the application starts, opens a project and the UI self-test passes inside it.

**M9 Differential check on real games.** A script runs the Python Core and the Rust Core on copies of the eight real games' translations (read-only originals) and compares the answers of the read operations (`overview`, `files`, `entries`, `validation`, `entry/menu` ...), normalised. Differences are fixed or explained one by one. *Done when:* no unexplained difference on the eight games.

**M10 Switch-over.** Installer from the Tauri bundler, the application of the release uses the Rust Core; the Python Core stays in the repository as the reference and for the golden tool until the owner decides to retire it.

## 6. Test strategy
1. **Golden data** (docs/GOLDEN.md) per module and per scenario: the main check, run on every PR (`cargo test`).
2. **Ported unit tests where a module has none in golden**: each milestone first adds golden cases on the Python side (inputs plus regenerated expected answers, reviewed), then ports.
3. **Differential run on the real games** (M9), and a **UI self-test** against the Rust server (M7) and inside the application (M8).
4. **Property checks** for the writer: a random edit followed by a re-parse gives the same entries (fuzzed inputs, also with CRLF and BOM), run on both cores.

## 7. Risks and what is done about them
| Risk | Why | What is done |
|---|---|---|
| Regular expressions | Python `re` has look-around and Unicode classes; the `regex` crate has no look-around (M1 needed none, so it uses the plain `regex` crate; `\w` and `\s` differ from Python for combining marks and U+001C to U+001F, no real file showed it) | `fancy-regex` where look-around is needed; every pattern is listed with the golden case that exercises it; differences found by the cases, not by memory |
| Python's `round`, `lower`, `sorted(key=str.lower)`, `str.isalpha` | different from Rust's rounding (banker's vs away from zero), case mapping, Unicode categories | golden cases with the values at the edges (percentages, Czech letters, mixed case file names); small helper functions that copy Python's behaviour |
| Pickle in `.rpa` | no standard Rust reader; a hostile index must never run code | a hand-written restricted reader of the subset Ren'Py uses; the hostile archive is in the golden data |
| Writing files | the one place that changes user data: line ends, BOM, atomic replace, backups, restore on failure, Windows file locking | the writer is its own milestone (M4), byte-identical `files_after`, the failed-write path is tested with a fake failure |
| Paths | Windows separators in file names and in answers | already normalised in the golden data; CI also on Linux from M1 |
| The amount of work | about 9 300 lines of Python (about 6 900 without the operations glue) | small milestones, each useful and checked; the order puts the most risky parts (parser, archives, writer) early |
| JSON | key order, number formatting (`1.0` vs `1`), non-ASCII escapes | the runner compares parsed JSON, not text; the answers the UI reads are compared by value |

## 8. How the UI reaches the Rust Core
**Pro features in the UI today (variant A, possibly temporary).** The UI of the five Pro features still lives in the Free UI. It asks Core which features are available (`features`, `ui/js/38-pro.js`) and, when one is not, shows a short "Pro feature" window instead of a failing call; the decision stays in Core and the pack. After the port the alternative is variant B: the pack ships its own UI pieces and Free only has a place for them (`ui_strings(code)` of the pack already carries their texts). It is decided with the sidecar question in section 9.

The interface is already one function: `call(op, params)` (`core/lexo/api.py`). In the browser build, `ui/js/10-api.js` turns it into `fetch('/api/<path>')`. In the Tauri build it will call `invoke('call', { op, params })` (and a second command for binary answers: fonts and pictures). Nothing else in the UI changes; the session routes (windows, focus, folder dialog, pings, fullscreen) become Tauri commands and events. The dev server of M7 keeps the HTTP form alive, which is what lets the existing UI self-test and CI run unchanged.

## 9. How the Pro pack connects after the port
Short answer: yes, the connection works, because it is only the plain-JSON call (`feature`, `method`, `payload`) of the extension host (PRO-DESIGN.md); the host is ported with Core (M6). Three things have to be true:
1. **The Rust Core builds the same requests.** The payloads the Python Core sends today (the candidate lines for the fixes, the layout in the request for group suggestions, the texts for regex replace, the requests for machine translation) are built in Core code. A later golden step records them with a recording provider (a fake pack that returns fixed answers), so that Rust is checked against the exact requests without any Pro code in `core/`.
2. **The pack runs without the Python library.** Today the pack imports a few Core helpers (`messages`, `util`, `textedit`, `store`, `layout`, `rpy`, `mt`, `export`). Options: (A) ship a Python runtime and the Python `lexo` library next to the exe for the pack only (simple, heavy, not "one exe without Python" for Pro users); (B) **port the pack to Rust as a closed crate that depends on `lexo-core`** (recommended: one toolchain, small, the five features are thin plain-data functions); (C) move the few helpers into a tiny shared library. If Core is published under the GPL, a pack linked against it is the owner's own program (the owner holds the copyright) but outside contributions to Core then need a contributor agreement; if Core stays closed there is no such question (PRO-DESIGN.md, sharing and licence).
3. **The transport.** The first Rust version calls the pack in process (a trait object, the same JSON in and out, so the rule "no live objects cross the border" holds); the sidecar (a separate program, JSON lines on stdin and stdout, Tauri has sidecars built in) replaces it without a change to any caller. Signature check, manifest updates and the licence are step 9 of the roadmap.
Until the pack is ported, the Rust Core answers the five Pro features as "locked", exactly as the Python Core does without the pack, and the application is a complete Free product.

## 10. Working agreement
- One milestone = one branch (`feat/rust-m1-parser` ...) and one PR with "what is not verified". The PR updates this document's Progress table and the ROADMAP log.
- A bug found in Python on the way: fix in Python on its own branch first, regenerate the golden answers, read the diff, merge, then port.
- No new feature, no renaming of an operation, no change of an answer inside a port PR (a changed golden answer needs its own explanation in the PR).
- Every milestone says in its PR which golden cases it covers and which parts of the module are not yet covered by any case.

## 11. Progress
| Milestone | State | PR |
|---|---|---|
| M0 Foundations | **done**: workspace `rust/` (`lexo-core` with `util::norm_ws`, `lexo-golden` with the normalisation, the inventory of 158 cases, the readable diff and the report), CI job `rust.yml`; the report is "0 of 158" | this PR |
| M1 Parser and archives | **done**: `messages` (msg, msgn, wire, render), `lang` (pack, `load_dir`, plural rules, stems), `util` (norm_ws, fold, sha1, the Python white-space rules), `pickle` (restricted reader), `rpa`, `rpy`; golden `rpy` 14 of 14 and `rpa` 6 of 6 (report: 20 of 158); the parser also gives the same answer as Python on 2 894 real translation files of 7 games (`rust/tools/diff_rpy_real_files.py`) | this PR |
| M2 Checks and layout | **done**: `layout` (parse, propose, folder and series numbers; `load` / `save_groups` wait for M3), `menus`, `validator` (all checks, spelling hints as a trait), `spelling`, `textedit` (plain, word, stem), `fontmetrics`, `fancy-regex` for look-around; new golden cases for spelling (10), textedit (19) and fontmetrics (6, tiny fonts); report 96 of 193; the checks and the spelling hints also give the same issues as Python on 2 894 real files (1 530 issues of 10 kinds, `--checks --spell`) | this PR |
| M3 Data files | **done**: `clock`, `csvio` (the CSV dialect of Python), `fsutil` (atomic writes, rotating backups), `pyfmt` (`json.dumps`, `round`, `float`, `casefold`), `config`, `datadir`, `lock`, `events`, `store`, `history`, `texthistory`, `memory` (with `difflib`), `snapshots` (zip); 222 data cases in the new data scenarios (report: 318 of 415); the store, history and events also read identically from the 9 real data folders of 8 games (`rust/tools/diff_data_real_folders.py`) | this PR |
| M4 Project and the writer | **done** (checked by data scenarios and real folders; the 97 steps of the four project scenarios need the operations of M6): `project` (discovery, refresh, renames, counts, proofread states, rows), `fixes` (the single writer: edit, many edits, planned fixes, undo stack, backups, CRLF and BOM kept, rollback), `export` (progress.json, its check, the target path), `bulk` (assignments, approval of identical lines, rules); 305 + 50 new data cases (report: 451 of 548); `lexo-golden --dump-project` reads 16 real translation folders of 8 games (1.4 million lines) identically to Python (`rust/tools/diff_project_real_folders.py`) | this PR |
| M5a Game tools: index, missed, lint, editor | **done**: `gameimages` (index of pictures and fonts in loose files and .rpa, layered pictures, characters, the picture behind a line), `missed` (texts the translation lacks, suggested functions, settings), `lint` (finding Python and the launcher, the command, parsing; the background thread waits for M6), `editor` (VS Code, Notepad++, custom command, fall back to the system); 100 new data cases (report: 551 of 648); `lexo-golden --dump-game` reads 8 real games identically to Python (`rust/tools/diff_game_real_folders.py`) | this PR |
| M5b Game tools: `gametools`, `mt` | **done**: `gametools` (safe names, unpacking with limits, the workspace of hard links, copying scripts, unrpyc and the Ren'Py generator through a `Runner`, repairing decompiled scripts, quarantining unparsable files, `prepare_translation`, a real `run_process` with a time limit), `mt` (placeholders, the requests and answers of DeepL, OpenAI and LibreTranslate through an `Http` function; `html.unescape` with the whole HTML 5 table generated from Python by `rust/tools/gen_html_entities.py`); 102 new data cases (report: 652 of 749); `lexo-golden --dump-tools` gives the same index, unpacked scripts, workspace and copied scripts as Python for 8 real games (`rust/tools/diff_tools_real_games.py`). The real HTTP client comes with the app (M8) | this PR |
| M6 The operations | **done**: the table of 80 operations (`api.rs`: `Param.coerce`, `call`, `describe`) equals `core/contract/api.json`; the application (`app*.rs`: open, refresh, persist, settings, validation, entries, glossary, proofreading, groups and assignments, approvals, memory, menus, game facts, snapshots, lint in a background thread, missed texts, fixes, replace, machine translation, publishing) and the extension host (`extensions.rs`, `features.rs`: the five gated features answer "locked" without a provider); all 97 steps of the four project scenarios and the files after them match (report: **749 of 749**); every operation with an example answers in the recorded shape (`core/contract/shapes.json`, `rust/crates/lexo-core/tests/contract.rs`). Differences from Python: the Pro pack (a Python package) cannot be loaded by the Rust host yet, `maybe_auto_snapshot` is not run in a background thread, the machine translation has no network client until M8 | this PR |
| M7 Dev server and the UI | planned | |
| M8 The Tauri application | planned | |
| M9 Differential check on real games | planned | |
| M10 Switch-over | planned | |
