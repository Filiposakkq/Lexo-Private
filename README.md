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
  │  rust/      rust/resources/ui  │      └────────────────────────────┘
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
| [`rust/`](rust/) | **The product.** Cargo workspace: `crates/lexo-core` (the library: parser, writer, checks, project, operations), `lexo-server` (local server for the UI: development and the UI self-test), `lexo-app` (the Tauri application), `lexo-golden` (checks the Rust core against the golden data); `tools/` (comparison scripts against real games) | product |
| [`rust/resources/ui/`](rust/resources/ui/) | **The user interface** (ES modules `js/NN-name.js`, `index.html`, CSS), shared by the Rust server and the Tauri app | product |
| [`rust/resources/packs/`](rust/resources/packs/) | **Language packs** (`cs`: rules, UI texts) | product |
| [`rust/resources/assets/`](rust/resources/assets/) | icons and the brand sheet | product |
| [`pro/rust/`](pro/rust/) | **The Pro pack** (private Cargo workspace): batch machine translation, safe fixes, regex replace, group suggestions, publishing, carry-over, fill from similar translations, profiles of characters, style check, the licence client; `pro/rust/lexo-pro/ui/` has its Czech texts | product (private) |
| [`pro/installer/`](pro/installer/) | the configuration of the Pro installer | product (private) |
| [`server/`](server/) | **Licence server** (TypeScript Worker + D1): activation, refresh, lost key, limits, admin page | service |
| [`web/`](web/) | **Website** and its small server (pages, applications, newsletter, admin) | service |
| [`testdata/`](testdata/) | **Recorded answers** the Rust code is tested against: `golden/` (the golden data), `contract/` (the interface of Core), `pro-golden/` (the Pro pack) | test data |
| [`reference/python/`](reference/python/) | **The Python implementation** (`core/`, `free/`, `pro/`): the reference the Rust port was checked against, the generator of the golden data, and the tests that guard the UI texts. Not shipped. | reference |
| [`docs/`](docs/) | the documents; start with [`docs/README.md`](docs/README.md) | docs |
| [`.github/workflows/`](.github/workflows/) | CI: Rust (`rust.yml`), Python and UI (`free.yml`), installers, server, website | CI |

## Where to start

- A new chat or a new person: [`docs/project/HANDOFF.md`](docs/project/HANDOFF.md) (decisions, rules, the loop of a step, commands, current state), then [`docs/project/ROADMAP.md`](docs/project/ROADMAP.md).
- The index of all documents and which ones are current: [`docs/README.md`](docs/README.md).
- What each Pro feature does: [`docs/guide/PRO-FEATURES.md`](docs/guide/PRO-FEATURES.md); working as a team: [`docs/guide/TEAMS.md`](docs/guide/TEAMS.md).

## Commands (Windows)

```
# Rust (the MSVC toolchain only; Rust lives on E:, the build output on D:)
set RUSTUP_HOME=E:/rust/rustup & set CARGO_HOME=E:/rust/cargo & set CARGO_TARGET_DIR=D:/rust-target-msvc
cd rust      & cargo +stable-x86_64-pc-windows-msvc test                       # Core, server
cd rust      & set LEXO_NO_PRO=1 & cargo +stable-x86_64-pc-windows-msvc run -q --locked -p lexo-golden     # the golden data: 749 of 749
cd pro\rust  & cargo +stable-x86_64-pc-windows-msvc test                       # the Pro pack
cd server    & npx.cmd vitest run & npx.cmd tsc --noEmit                       # the licence server
cd web\worker & npx.cmd vitest run                                              # the website server

# Python (the reference; also the guards of the UI texts: every text has a Czech translation, no stale texts, the UI calls only existing operations)
py reference/python/run_tests.py core | free | pro | (no argument = all)

# running the UI against the Rust server (use LEXO_UI_DIR: a stale copy of the UI may sit next to the program in target/)
set LEXO_UI_DIR=rust\resources\ui & set LEXO_PACKS_DIR=rust\resources\packs
rust\target\debug\lexo-server.exe --project "D:\games\MyGame\game\tl\czech"
```

The old BaDIK tool stays where it is (`Being-A-DIK-CZ/tools/translation-manager`) and keeps working for the Czech translation of Being a DIK.
