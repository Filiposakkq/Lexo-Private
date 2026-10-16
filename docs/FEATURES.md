# Features: Free or Pro (proposal, revision 2)

Principle: the **whole translating workflow is free** (so people can really use and recommend the app). Pro sells
**time savers for big or long projects, automation, release tooling and teamwork**. A feature goes to Pro only if the free
app still works fully without it, and nothing that users see as a basic right is locked (find & replace, spelling, history).

Revision 2 follows a review of revision 1 (find & replace, spelling and line diff move to Free; four pain points are added to Pro).
Module = where the code lives today in `free/lexo/`. "Coupling" = how hard it is to pull the feature out later.

## Free
| Feature | Module | Coupling |
|---|---|---|
| Open project, file list, line editor, filters, search, undo, backups (snapshots) | `project`, `store`, `rpy`, `snapshots` | core |
| History of a line **and the diff between its versions** | `history`, `texthistory` | core |
| Proofreading state, quick translate/proofread mode, focus mode | `store`, `app`, UI | core |
| Validation: tags, variables, brackets, length, dialogue-box fit; language packs | `validator`, `fontmetrics`, `lang` | core |
| **Find & replace across the project** (plain text) | `textedit` | only `app.py` |
| **Spelling hints** from the project's own vocabulary (basic) | `spelling` | `app.py` |
| Scene preview with game images and fonts | `gameimages`, `rpa`, `fontmetrics` | only `app.py` |
| Glossary, import of character names, term check | `app`, `validator` | core |
| Ren'Py lint with the SDK, open in editor | `lint`, `editor` | only `app.py` |
| Weekly goals, completion estimate, per-group progress | `goals`, `export` | `export` |
| Machine translation of single lines (own API key) | `mt` | only `app.py` |
| Approve identical lines (the basic form) | `bulk` | only `app.py` |
| Translation file generator, unrpa / unrpyc helpers (planned) | new | new |

## Pro
Aimed at people with bigger projects, a website or Patreon, or a team. Priced low: a one-off licence or a small yearly one.

| Feature | Why people pay for it | Module | Coupling |
|---|---|---|---|
| **Carry a translation over to a new game version** (fuzzy matching of old translations to changed or moved originals, marked "check") | A game update shifts lines and the translator redoes it by hand; this saves the most work of all | new (builds on `memory`, `rpy`) | new |
| **Fuzzy matches in translation memory** (not only identical originals) | Repeated and similar lines across a long game | `memory` | only `app.py` |
| **Style and consistency check with an LLM** (formal/informal address, gender, character voice) | Machine translation often mixes formal and informal address; where the language has such rules (for example Czech tykání/vykání) the check knows them through the language pack | new, builds on `mt` and `lang` | new |
| **Character profiles** (who addresses whom how, gender, speech style) passed to batch machine translation | Makes the batch output usable | new + `mt` | new |
| Batch machine translation with glossary, context and the profiles | Speed | `mt`, `bulk` | only `app.py` |
| **Release builder**: translation -> installable patch (zip or rpa), version, changelog | "From translation to release" in one tool, together with publishing | new | new |
| Publishing progress (website / JSON) | Translators with a site or Patreon | `export`, `publish` | only `app.py` |
| **Safe bulk fixes** (tag- and variable-aware, with a preview of every change) and **regex batches** | Big clean-ups without breaking tags | `fixes` | only `app.py` |
| Bulk categorisation suggestions (assign files to groups) | Large multi-episode projects | `bulk` | only `app.py` |
| Smarter spelling: extra language packs and better suggestions | Quality for more languages | `spelling`, `lang` | `app.py` |
| Team work: several translators, assignments, who did what, merging changes (planned) | Teams | new | new |
| Reports (CSV/PDF), saved filter views (ideas) | Nice to have | new | new |

## Language packs
The app and its UI are **English first**. Everything that depends on the language of the translation lives in a **language pack**
(`free/lexo/lang.py` today: repeated words, number words, stems; later also address rules for the style check, spelling data, UI texts).
- **Czech is the first pack**, built the same way as any other, not special-cased in the app. It is the reference for how a pack is made.
- Packs are **separate downloadable modules** (installed on demand, updated independently of the app). The app works with the neutral rules when no pack is installed.
- Basic packs (rules for the free checks) are free; packs with the style check data (address, gender, voice) belong to the Pro style check.
- Done: the pack format and loader (data files `pack.json` + `rules.json`, bundled and in `<config>/packs`; Czech is `free/lexo/packs/cs`). The UI string table is done (English source text, Czech as `ui.json` of the pack; see UI-TEXTS.md). To do: a pack list and download in the app, `address.json` / `spelling.txt` parts, and message keys for the Python-side messages.

## Taste before paying
- **Limits, not a locked button.** A Pro feature in Free works on a small amount (for example batch translation or safe fixes for 20 lines per run) so that the user sees it works; Pro lifts the limit.
- **Show the value before the price.** Example text in the UI: "This line has 37 identical originals, Pro approves them at once." The number is the sales argument.
- The limit is applied in the Pro module, not scattered through Core: Core only asks "which features are available, and with which limit?".
- A limit per run is only a taste: the user can click again. It is not protection. What makes removing a limit costly is that **the free version is a simpler, separate implementation** (for example batch translation without context, glossary or parallel batches), not the Pro code with a constant `limit=20`. Free code must not contain the full logic.

## Licensing and protection (decision)
Status: **decided in principle, built at release (roadmap step 6)**. The source code stays **private** for now; whether any
part becomes public is decided before the first release.

- **Goal:** make bypassing cost more than the licence. A crack will appear if the app becomes popular; that is accepted. Pro value is convenience, updates (new language packs) and support.
- **Pro code is not shipped without a licence.** The Pro module is downloaded after a valid key is entered. Free contains only the deliberately limited version of a feature (e.g. 20 lines), so removing the limit means rewriting the feature, not flipping a flag.
- **Licence key:** signed (Ed25519), verified offline by the app.
- **Check every 30 days:** at start-up the app asks the licence server whether the key is still valid and stores the answer, signed by the server, so that it cannot be forged with a file.
  - If the check cannot be made (no internet), a **14-day grace period** applies with a notice; Pro never switches off in the middle of the work of an honest user.
  - If the licence is cancelled or refunded, only the Pro features switch off. Projects and data are never locked or deleted, and Free keeps working.
- **Two devices per key:** a device registers on activation (anonymous installation id stored at activation, no personal data and no hardware serial number, so a new disk or a Windows update does not break it). The user can **deactivate a device** in the app and on a small web page, so a new computer never leaves them locked out.
- **Server:** a small service (for example a Cloudflare Worker with storage) holds key, devices, last check and status; a payment provider that issues keys and sends webhooks (Lemon Squeezy, Gumroad, Paddle and similar) handles payment and cancellation. This is the first part that needs operation (availability, backups, privacy and terms of use) - plan for it before release.
- **Not doing:** obfuscation or packers (antivirus false positives hit honest users), hardware binding, always-online checks.

In the code (already built): `features.py` is the catalog of gated features and the `Entitlements` object; the application asks it at the few
places where a Pro feature is used. Today the default is everything on (development); the licence loader will later provide the real answer.

## Can it be separated later?
Yes. Every module marked "only `app.py`" is imported from nowhere else, so it can move to `pro/` and be loaded through a registry
instead of a direct import. What has to be built first (roadmap step 2):
1. a **feature registry** in Core (`register("mt.batch", handler, limit=...)`) so `app.py` and the UI ask "is this available, and with which limit?";
2. **UI hooks**: a Pro feature shows when the registry has it, otherwise a short hint with the value ("37 identical originals");
3. the **licence check** only in the Pro loader.

Moving a feature from Free to Pro after users have it is unpopular, so decide before the first public release.

## Open questions
- Price and form of the licence (one-off vs yearly).
- LLM features need an API key; the user brings it (no cost for us), or a hosted option later.
- Licence of the public part (e.g. MIT or GPL); it decides how much of Free others can reuse. **Decide before the first public release**, because it also decides what may be in the Free code (licence logic, limits).
