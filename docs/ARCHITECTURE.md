# Architecture (draft)

```
Pro modules (private)  ──┐
                         ├──> Core interface <── Free app (UI)
Language packs ──────────┘         │
                                   ├── project model, parser, writer
                                   ├── lint, fonts, scene data
                                   └── settings, snapshots, history
```

## Layers
- **Core**: no UI, no network server. Everything the app does is a function of Core. A single place writes `.rpy` files (as in the old tool, `fixes.py`).
- **Free**: the application. Today it talks to Core through a local JSON API; after the Rust port it will call Core directly.
- **Language packs**: `lang/<code>` with checks and spelling for a language; Core keeps neutral checks only.
- **Pro modules**: register themselves in Core through the extension points below; Core works without them.

## Core interface (first sketch, to refine in step 2)
`open_project(path)`, `refresh()`, `files()`, `lines(file, filter)`, `set_translation(line, text)`, `mark_proofread(line)`,
`validate(line)`, `search(query)`, `snapshot()`, `scene(line)`, `generate_translation(game, language)`.

## Extension points for Pro
`translate_provider` (machine translation), `memory_provider` (translation memory), `exporter` (progress/web), `team_sync`.
A Pro module only adds an implementation; the UI shows the feature when the module is present and licensed.

## Free / Pro split (proposal)
- **Free**: the whole translating workflow (see docs/FEATURES.md): editor, proofreading, validation, find & replace, spelling, history with diff, scene preview, statistics, translation generator.
- **Pro**: carrying translations over to a new game version, fuzzy translation memory, LLM style check, character profiles, batch machine translation, release builder, safe bulk fixes and regex batches, publishing, teamwork.

## Licensing and basic protection
Signed licence key (public key in the app, private key only with the author), checked offline, optional online refresh later.
Pro code is never in the public repository. Beyond that nothing heavy: it cannot be made unbreakable, so keep it simple and fair.
