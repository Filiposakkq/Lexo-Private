# Lexo (private)

Translation manager for Ren'Py games: edit, proofread, check and track translations of any Ren'Py project.
Working name. **This repository is private**; the application is described in `free/README.md`.

> Private monorepo. `pro/` never leaves this repository. Whether `core/` and `free/` are published (open source) or shipped as closed freeware is the owner's open decision (docs/ROADMAP.md); if they are published, they are copied without history.
> Run the tests with `py run_tests.py` (Core, Free, Pro).

## Layout

| Folder | What goes there | Becomes public? |
|---|---|---|
| `core/` | Package `lexo`: the library without UI or HTTP: reading and writing `.rpy` and `.rpa`, checks, project model, settings, the operations table (`api.py`), the extension host, language packs; its tests, the generated contract and the golden data | to be decided (see docs/ROADMAP.md, open decisions) |
| `free/` | Package `lexo_free`: the application around Core (local server, window, browser UI); its tests | to be decided |
| `pro/` | Package `lexo_pro`: the Pro pack (batch machine translation, safe fixes, regex replace, group suggestions, publishing, the limits of the demo); its tests | no, never |
| `docs/` | Architecture, interface, golden data, Pro design, plan, audits | curated |

## Where to start
0. [docs/HANDOFF.md](docs/HANDOFF.md) – start here in a new chat: decisions, rules, the step loop, commands.
1. [docs/ROADMAP.md](docs/ROADMAP.md) – the plan, in order, with what "done" means for each step.
2. [docs/HARDCODED-AUDIT.md](docs/HARDCODED-AUDIT.md) – everything in the old tool that is tied to Being a DIK or to Czech.
3. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) – layers, the Core interface, how Free and Pro attach.
4. [docs/PRO-DESIGN.md](docs/PRO-DESIGN.md) – how Pro is kept apart from the public code, delivered and updated (and what is still open).
5. [docs/API.md](docs/API.md) – the interface of Core (generated), [docs/GOLDEN.md](docs/GOLDEN.md) – the golden data the Rust port is checked against.
6. [docs/DATA-LAYOUT.md](docs/DATA-LAYOUT.md), [docs/TEAMWORK.md](docs/TEAMWORK.md), [docs/FEATURES.md](docs/FEATURES.md), [docs/PROJECT-FILE.md](docs/PROJECT-FILE.md), [docs/UI-TEXTS.md](docs/UI-TEXTS.md), [docs/GAME-TOOLS.md](docs/GAME-TOOLS.md).

The old tool stays where it is (`Being-A-DIK-CZ/tools/translation-manager`) and keeps working for the Czech translation.
