# Aside (code name Lexo)

A translation manager for Ren'Py games: edit, proofread, check and track the translation of any Ren'Py project. The product is called **Aside**; the repository, the crates
and many paths still say **Lexo** (the first working name), which is intentional: renaming them would only make the history harder to follow.

> **Private monorepo.** `pro/` never leaves it. The repository is *temporarily public*; until it is private again, do not push premium Pro code that is not already here.

## What the product is made of

```
 you (Ren'Py translator)
        │
  ┌─────▼──────────────────────────┐      ┌────────────────────────────┐
  │ Aside application (Tauri)      │─────▶│ licence server (server/)   │  activate, refresh, lost key
  │  Rust core  +  web UI          │      │  Cloudflare Worker + D1    │
  │  rust/            free/…/ui    │      └────────────────────────────┘
  └─────▲──────────────────────────┘
        │ JSON lines (stdin/stdout)
  ┌─────┴──────────────────────────┐      ┌────────────────────────────┐
  │ Pro pack = sidecar program     │      │ website (web/)             │  pages, forms, admin
  │  pro/rust/                     │      │  Cloudflare Worker + D1    │
  └────────────────────────────────┘      └────────────────────────────┘
```

## Folders

| Folder | What it is | Kind |
|---|---|---|
| [`rust/`](rust/) | **The product.** Cargo workspace: `lexo-core` (the library: parser, writer, checks, project, operations), `lexo-server` (local server for the UI, used for development and the UI self-test), `lexo-app` (the Tauri application), `lexo-golden` (checks the Rust core against the golden data), `tools/` (comparison scripts against real games) | product |
| [`free/lexo_free/ui/`](free/lexo_free/ui/) | **The user interface** (ES modules `js/NN-name.js`, `index.html`, CSS). Shared by the Rust server and the Tauri app. | product |
| [`core/lexo/packs/`](core/lexo/packs/) | **Language packs** (`cs`, rules and UI texts). Used by the Rust core. | product |
| [`free/assets/`](free/assets/) | icons and the brand sheet | product |
| [`pro/rust/`](pro/rust/) | **The Pro pack** (private Cargo workspace): batch machine translation, safe fixes, regex replace, group suggestions, publishing, carry-over, the licence client | product (private) |
| [`pro/installer/`](pro/installer/) | the configuration of the Pro installer | product (private) |
| [`server/`](server/) | **Licence server** (TypeScript Worker + D1): activation, refresh, lost key, limits, admin page | service |
| [`web/`](web/) | **Website** and its small server (pages, applications, newsletter, admin) | service |
| [`core/`](core/) and [`free/`](free/) (without the parts above), [`pro/lexo_pro/`](pro/lexo_pro/), [`pro/tests/`](pro/tests/) | **The Python implementation**: the reference the Rust port was checked against, the generator of the golden data and the tests that guard the UI texts. Not shipped. | reference |
| [`core/golden/`](core/golden/), [`core/contract/`](core/contract/), [`pro/golden/`](pro/golden/) | **Recorded answers** (golden data, the interface contract) that the Rust code is tested against | test data |
| [`docs/`](docs/) | the documents; start with [`docs/README.md`](docs/README.md) | docs |
| [`.github/workflows/`](.github/workflows/) | CI: Rust (`rust.yml`), Python and UI (`free.yml`), installers, server, website | CI |

Why `core/` and `free/` hold both reference code and product files: the product was a Python application first, and the Rust port moved the code but kept using the same
UI, language packs, golden data and brand files from their old places. Moving them is a planned clean-up (see [`docs/REPO-CLEANUP.md`](docs/REPO-CLEANUP.md)).

## Where to start

- A new chat or a new person: [`docs/HANDOFF.md`](docs/HANDOFF.md) (decisions, rules, the loop of a step, commands, current state), then [`docs/ROADMAP.md`](docs/ROADMAP.md).
- The index of all documents and which ones are current: [`docs/README.md`](docs/README.md).

## Commands (Windows)

```
# Rust (the MSVC toolchain only; Rust lives on E:, the build output on D:)
set RUSTUP_HOME=E:/rust/rustup & set CARGO_HOME=E:/rust/cargo & set CARGO_TARGET_DIR=D:/rust-target-msvc
cd rust      & cargo +stable-x86_64-pc-windows-msvc test                       # Core, server
cd rust      & set LEXO_NO_PRO=1 & cargo +stable-x86_64-pc-windows-msvc run -q --locked -p lexo-golden     # the golden data: 749 of 749
cd pro\rust  & cargo +stable-x86_64-pc-windows-msvc test                       # the Pro pack
cd server    & npx.cmd vitest run & npx.cmd tsc --noEmit                       # the licence server
cd web\worker & npx.cmd vitest run                                              # the website server

# running the UI against the Rust server (use LEXO_UI_DIR, a stale copy of the UI may sit next to the program)
set LEXO_UI_DIR=free\lexo_free\ui & set LEXO_PACKS_DIR=core\lexo\packs
rust\target\debug\lexo-server.exe --project "D:\games\MyGame\game\tl\czech"

# Python (reference and the guards of the UI texts; CI runs them: py run_tests.py)
```

The old BaDIK tool stays where it is (`Being-A-DIK-CZ/tools/translation-manager`) and keeps working for the Czech translation of Being a DIK.
