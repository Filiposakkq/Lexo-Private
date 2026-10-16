# Lexo (private)

Translation manager for Ren'Py games: edit, proofread, check and track translations of any Ren'Py project.
Working name; the product grows out of the BaDIK Translation Manager (a Python tool built for *Being a DIK CZ*).

> Private monorepo. When Lexo is released, only `core/` and `free/` are copied (without history) into a new
> public repository. `pro/` never leaves this repository.

## Layout

| Folder | What goes there | Becomes public? |
|---|---|---|
| `core/` | Library without UI: reading/writing `.rpy`, `.rpa`, lint, font metrics, project model, settings, plug-in interface | yes |
| `free/` | The application (UI + wiring to Core) | yes |
| `pro/` | Paid modules (e.g. machine translation in batches, translation memory, team work, licence check) | no |
| `docs/` | Architecture, plan, audit of what is still tied to one game | yes (curated) |

## Where to start
0. [docs/HANDOFF.md](docs/HANDOFF.md) – start here in a new chat: decisions, rules, the step loop, commands.
1. [docs/ROADMAP.md](docs/ROADMAP.md) – the plan, in order, with what "done" means for each step.
2. [docs/HARDCODED-AUDIT.md](docs/HARDCODED-AUDIT.md) – everything in the old tool that is tied to Being a DIK or to Czech.
3. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) – layers, the Core interface, how Free and Pro attach.
4. [docs/DATA-LAYOUT.md](docs/DATA-LAYOUT.md), [docs/TEAMWORK.md](docs/TEAMWORK.md), [docs/FEATURES.md](docs/FEATURES.md), [docs/PROJECT-FILE.md](docs/PROJECT-FILE.md), [docs/UI-TEXTS.md](docs/UI-TEXTS.md), [docs/GAME-TOOLS.md](docs/GAME-TOOLS.md).

The old tool stays where it is (`Being-A-DIK-CZ/tools/translation-manager`) and keeps working for the Czech translation.
