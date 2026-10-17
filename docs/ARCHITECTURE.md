# Architecture (draft)

```
Pro modules (private)  ──┐
                         ├──> Core interface <── Free app (UI)
Language packs ──────────┘         │
                                   ├── project model, parser, writer
                                   ├── lint, fonts, scene data
                                   └── settings, snapshots, history
```

## Where things are (after the split of Core and Free)
```
core/lexo/        package `lexo` = Core: project model, parser, writer, checks, App (app_*.py), language packs (packs/), game tools. Imports no UI, no HTTP.
core/tests/       Core tests (353): run with `py run_tests.py core`; Free is not on the import path
free/lexo_free/   package `lexo_free` = Free: server.py (local HTTP JSON server), cli.py (window, command line), ui/ (the browser UI)
free/tests/       Free tests: the HTTP API, the UI files, the command line; `py run_tests.py free`
free/run_lexo.py  launcher (puts core/ and free/ on the import path), Lexo.spec / build_exe.bat build the exe from both
```
Rules: Core never imports `lexo_free`; `lexo_free` imports Core as `lexo` (absolute imports). The language packs live in Core (`core/lexo/packs`): a pack is rules for the Core
checks plus the UI texts, and the server hands the UI texts out. The tests that go through the HTTP API (`free/tests`) are the first form of the interface contract (roadmap step 5).

## Layers
- **Core**: no UI, no network server. Everything the app does is a function of Core. A single place writes `.rpy` files (as in the old tool, `fixes.py`).
- **Free**: the application. Today it talks to Core through a local JSON API; after the Rust port it will call Core directly.
- **Language packs**: `lang/<code>` with checks and spelling for a language; Core keeps neutral checks only.
- **Pro modules**: register themselves in Core through the extension points below; Core works without them.

## Core interface (first sketch, to refine in step 2)
`open_project(path)`, `refresh()`, `files()`, `lines(file, filter)`, `set_translation(line, text)`, `mark_proofread(line)`,
`validate(line)`, `search(query)`, `snapshot()`, `scene(line)`, `generate_translation(game, language)`.

## Extension points for Pro
Built (see PRO-DESIGN.md): `core/lexo/extensions.py` is the host. A Pro feature is a **JSON call** (`host.call("publish.web", "plan", {...})`) answered by the separately delivered
Pro pack (`pro/lexo_pro`); without the pack the feature is locked and no flag in Core or Free can change that. The first extracted feature is `publish.web`; four more are still
in Core behind the old flag (`Feature.extracted = False` in `features.py`), listed in PRO-DESIGN.md.
Planned names for further features: `translate_provider` (machine translation batches), `memory_provider` (fuzzy translation memory), `exporter`, `team_sync`.
The UI shows a feature when the pack offers it, otherwise a locked hint.

## Free / Pro split (proposal)
- **Free**: the whole translating workflow (see docs/FEATURES.md): editor, proofreading, validation, find & replace, spelling, history with diff, scene preview, statistics, translation generator.
- **Pro**: carrying translations over to a new game version, fuzzy translation memory, LLM style check, character profiles, batch machine translation, release builder, safe bulk fixes and regex batches, publishing, teamwork.

## Licensing and basic protection
Signed licence key (public key in the app, private key only with the author), checked offline, optional online refresh later.
Pro code is never in the public repository. Beyond that nothing heavy: it cannot be made unbreakable, so keep it simple and fair.
