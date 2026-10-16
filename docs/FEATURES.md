# Features: Free or Pro (proposal, to be decided by the owner)

Principle: the **whole translating workflow is free** (so people can really use and recommend the app); Pro sells
**time savers, automation, publishing and teamwork**. A feature goes to Pro only if the free app still works fully without it.

Module = where the code lives today in `free/lexo/`. "Coupling" says how hard it is to pull the feature out later.

| Feature | Module | Proposal | Coupling today |
|---|---|---|---|
| Open project, file list, line editor, filters, search, undo, history of a line, backups (snapshots) | `project`, `store`, `rpy`, `history`, `texthistory`, `snapshots` | **Free** | core |
| Proofreading state (✓, ⚑, stale marks), quick translate/proofread mode, focus mode | `store`, `app`, UI | **Free** | core |
| Validation: tags, variables, brackets, length, dialogue-box fit | `validator`, `fontmetrics` | **Free** | core |
| Language rules (repeated words, stems) | `lang` | **Free** (packs for more languages) | core |
| Scene preview with game images and fonts | `gameimages`, `rpa`, `fontmetrics` | **Free** (the showpiece) | only `app.py` |
| Glossary, import of character names, term check | `app`, `validator` | **Free** | core |
| Ren'Py lint with the SDK, open in editor | `lint`, `editor` | **Free** | only `app.py` |
| Weekly goals, completion estimate, per-group progress | `goals`, `export` | **Free** (basic) | `export` |
| Translation file generator, unrpa / unrpyc helpers (planned) | new | **Free** | new |
| Machine translation (single line) | `mt` | **Free**, bring your own key | only `app.py` |
| Machine translation in batches with glossary and context | `mt`, `bulk` | **Pro** | only `app.py` |
| Translation memory and consistency by identical original, "approve all identical" | `memory`, `bulk` | **Pro** (memory), basic approve **Free** | only `app.py` |
| Safe fixes and find & replace across the project | `fixes`, `textedit` | **Pro** | only `app.py` |
| Bulk categorisation suggestions (assign files to groups) | `bulk` | **Pro** | only `app.py` |
| Spelling hints from the project's own vocabulary | `spelling` | **Pro** (language packs make it better) | `app.py` |
| Publishing progress to a website / JSON export | `export`, `publish` | **Pro** | only `app.py` |
| Team work: several translators, assignments, who did what, merge of changes (planned) | new | **Pro** | new |
| Statistics reports (CSV/PDF), saved filter views, diff of line versions (ideas) | new | **Pro** | new |

## Can it be separated later?
Yes, and it is cheap for the rows above: every module marked "only `app.py`" is imported from nowhere else, so it can be
moved to `pro/` and loaded through a registry instead of a direct import. What has to be built first (roadmap step 2):
1. a **feature registry** in Core (`register("mt.batch", handler)`), so `app.py` and the UI ask "is this feature available?"
   instead of importing the module;
2. **UI hooks**: the UI shows a Pro feature only when the registry has it (otherwise a short "Pro" hint);
3. the **licence check** in the Pro loader only. Core and Free never contain licence code beyond "is Pro present".

Until then everything lives together in `free/`; nothing needs to be decided now. The table can be changed any time before release,
but moving a feature from Free to Pro after users have it free is unpopular, so decide before the first public release.
