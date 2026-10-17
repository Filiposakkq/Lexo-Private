# Features: Free or Pro (proposal, revision 2)

Principle: the **whole translating workflow is free** (so people can really use and recommend the app). Pro sells
**time savers for big or long projects, automation, release tooling and teamwork**. A feature goes to Pro only if the free
app still works fully without it, and nothing that users see as a basic right is locked (find & replace, spelling, history).

Revision 2 follows a review of revision 1 (find & replace, spelling and line diff move to Free; four pain points are added to Pro).
Module = where the code lives today in `core/lexo/`. "Coupling" = which other code imports the module (**not** whether the feature is Free or Pro): "only `app.py`" means nothing else uses it, so it can be lifted out later. How Pro features are kept apart and delivered: PRO-DESIGN.md.

## Free

Status as of the Core / Free split and the interface contract (roadmap steps 1 to 5). **done** = works and is tested; **partly** = works with a stated gap; **done in code** = the code and its tests exist, the UI does not.

| Feature | Module | Coupling | Status |
|---|---|---|---|
| Open project, file list, line editor, filters, search (with a **whole word** switch), undo, backups (snapshots) | `project`, `store`, `rpy`, `snapshots` | core | **done** |
| History of a line **and the diff between its versions** | `history`, `texthistory` | core | **done** |
| Proofreading state, quick translate/proofread mode, focus mode | `store`, `app`, UI | core | **done** |
| Validation: tags, variables, brackets, length, dialogue-box fit; language packs | `validator`, `fontmetrics`, `lang` | core | **done** (packs: Czech and a neutral one; more languages = more packs) |
| **Find & replace across the project** (plain text; regex is Pro) | `textedit` | only `app.py` | **done** |
| **Spelling hints** from the project's own vocabulary (basic) | `spelling` | `app.py` | **done** (no dictionary; works for any language) |
| **Results bar in the film layouts** (quick translating / proofreading): with a search or filter active, "Result 3 of 16", previous / next (← →) and a list of all hits to pick from; the first hit opens by itself | UI | UI only | **done** |
| **Menu of a choice**: the question (or the lines said before the menu), all choices with their translations and conditions, and the first lines after each choice, read from the game script; marks a text shared with an earlier menu | `menus`, `app_menu` | only `app.py` | **done** (also in the old BaDIK tool) |
| Scene preview with game images (the background and the pictures shown over it) and fonts | `gameimages`, `rpa`, `fontmetrics` | only `app.py` | **partly**: layered images (`layeredimage`, composites) are not drawn yet, the preview says so |
| Glossary, import of character names, term check | `app`, `validator` | core | **done** |
| Ren'Py lint with the SDK, open in editor | `lint`, `editor` | only `app.py` | **done** |
| Weekly goals, completion estimate, per-group progress | `goals`, `export` | `export` | **done** |
| Machine translation of single lines (own API key) | `mt` | only `app.py` | **done** |
| Approve identical lines (the basic form) | `bulk` | only `app.py` | **done** |
| Translation file generator, unrpa / unrpyc helpers | `gametools` | `gametools` | **done in code** (tested on 8 games); **no UI yet** (planned with the new UI) |
| **Preparing a built game** (workspace of links, scripts out of the archives, unrpyc, generating with the game's own engine) | `gametools`, `lint` | `gametools` | **done in code**; **no UI yet** |
| **Texts the generator misses** (`show text`, input prompts, titles in variables; the project's own text functions; ignore list) - **experimental**: a heuristic list that can be wrong either way and must be reviewed, added to `extra_strings.rpy` (putting each text into the file of its own script is a further option) | `missed` | `app_missed` | **done** (experimental; also in the old BaDIK tool) |
| **Groups of a project from file names** (episode, chapter, part, day, update, act, `script`, `script2` ... series, and one group per folder), group word in all labels, suggested assignment of files | `layout`, `bulk` | `layout` | **done** |
| Projects of any game: searches stay in the game's own `game` folder, a game folder opens its translation, source scripts are not translation files | `project`, `app_*` | core | **done** (tried on 8 games) |

## Pro
Aimed at people with bigger projects, a website or Patreon, or a team. Priced low: a one-off licence or a small yearly one.

**In the Pro pack?** *yes* = the code is in `pro/lexo_pro` (private), the feature is locked in Free whatever flags are changed, and its limits belong to the pack. The cheap *taste* features (batch machine translation, safe fixes, regex replace, categorisation) will ship inside the installer as a demo pack with limits, so the demo needs no extra download; the *premium* ones are downloaded only after a licence (PRO-DESIGN.md). *not built* = does not exist yet; when it is built it is written in the pack from the start.


| Feature | Why people pay for it | Module | Coupling | In the Pro pack? |
|---|---|---|---|---|
| **Carry a translation over to a new game version** (fuzzy matching of old translations to changed or moved originals, marked "check") | A game update shifts lines and the translator redoes it by hand; this saves the most work of all | new (builds on `memory`, `rpy`) | new | not built |
| **Fuzzy matches in translation memory** (not only identical originals) | Repeated and similar lines across a long game | `memory` | only `app.py` | not built |
| **Style and consistency check with an LLM** (formal/informal address, gender, character voice) | Machine translation often mixes formal and informal address; where the language has such rules (for example Czech tykání/vykání) the check knows them through the language pack | new, builds on `mt` and `lang` | new | not built |
| **Character profiles** (who addresses whom how, gender, speech style) passed to batch machine translation | Makes the batch output usable | new + `mt` | new | not built |
| Batch machine translation with glossary, context and the profiles | Speed | `mt`, `bulk` | only `app.py` | **yes** – `pro/lexo_pro/mt_batch.py` (`mt.batch`; one line of machine translation stays free in Core) |
| **Release builder**: translation -> installable patch (zip or rpa), version, changelog | "From translation to release" in one tool, together with publishing | new | new | not built |
| Publishing progress (website / JSON) | Translators with a site or Patreon | `export` (Core: the local progress file), `publish` (pack) | only `app.py` | **yes** – `pro/lexo_pro/publish.py` (`publish.web`) |
| **Safe bulk fixes** (tag- and variable-aware, with a preview of every change) and **regex batches** | Big clean-ups without breaking tags | `fixes` | only `app.py` | **yes** – `pro/lexo_pro/fix_safe.py` (`fixes.safe`) and `regex_replace.py` (`replace.regex`); plain find & replace stays free |
| Bulk categorisation suggestions (assign files to groups) | Large multi-episode projects | `bulk` | only `app.py` | **yes** – `pro/lexo_pro/categorize.py` (`bulk.categorize`; applying a chosen assignment stays free) |
| Smarter spelling: extra language packs and better suggestions | Quality for more languages | `spelling`, `lang` | `app.py` | not built |
| Team work: several translators, assignments, who did what, merging changes (planned) | Teams | new | new | not built |
| Reports (CSV/PDF), saved filter views (ideas) | Nice to have | new | new | not built |

## Language packs
A pack also lists the `letters` of the language (used to tell a translated line from an untranslated one), the repeated words that are normal in speech, the stemming rule and the plural rule; see `lang.py`.
The app and its UI are **English first**. Everything that depends on the language of the translation lives in a **language pack**
(`core/lexo/lang.py` today: repeated words, number words, stems; later also address rules for the style check, spelling data, UI texts).
- **Czech is the first pack**, built the same way as any other, not special-cased in the app. It is the reference for how a pack is made.
- Packs are **separate downloadable modules** (installed on demand, updated independently of the app). The app works with the neutral rules when no pack is installed.
- Basic packs (rules for the free checks) are free; packs with the style check data (address, gender, voice) belong to the Pro style check.
- Done: the pack format and loader (data files `pack.json` + `rules.json`, bundled and in `<config>/packs`; Czech is `core/lexo/packs/cs`). The UI string table is done (English source text, Czech as `ui.json` of the pack; see UI-TEXTS.md). To do: a pack list and download in the app, `address.json` / `spelling.txt` parts, and message keys for the Python-side messages.

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

## How Pro gets into Free (flow, decided in principle)
What the user sees: enters a key in Settings, the Pro features appear, nothing is reinstalled by hand.

1. **Free build** contains the gate (`features.py`), the licence-key check and the deliberately limited Free version of each gated feature (written as its own simpler implementation, not as the Pro code with a flag). It contains **no Pro code**.
2. **Key entered:** the app verifies the Ed25519 signature offline (public key inside the app). A malformed or forged key is refused before any network call.
3. **Activation:** the app registers the device with the licence server (anonymous installation id, two devices per key) and receives a signed answer (valid until, features, module version to use).
4. **Download:** the server hands out the Pro module for this app version only for a valid licence. The package is **signed**; the app verifies the signature before it stores it in `<config>/pro/` and again before every load (a modified package is ignored with a notice).
5. **Load:** the Pro loader loads the module and registers its features in the registry; from then on `features.py` answers "available" and the Pro implementation replaces the Free one. The UI shows the Pro features because the registry has them.
6. **Every 30 days** the licence is re-checked (14 days grace without internet). When it lapses, only the loader stops loading the module: projects and data stay, Free keeps working, the downloaded package may stay on disk.
7. **App updates and the module:** the module declares which app versions it fits; after an app update the loader downloads a matching module (or says that none exists yet) and never loads an incompatible one.

Where it attaches in the code (Python today): `App` is assembled from parts (`app_*.py`). A Pro part is one more mixin added to `App` by the loader. Parts that are Pro only
(`app_mt.py`, `app_publish.py`) can move to `pro/` as they are; parts with a Free and a Pro behaviour (fixes, replace, bulk assignment) need their Free variant written separately first.
The gates that exist now (`_gate`, `_cap` in `app_core.py`) are the places to replace by the registry.

**Open until the start of the Rust port** (decided then, not before): how the Pro module is loaded in the Tauri app. Candidates:
- a **sidecar**: Pro is a separate program that the app starts and talks to through a local interface; needs the clean UI-Core interface of roadmap step 5 anyway (current favourite);
- a **dynamic library** loaded at run time (fast, but fragile across versions and platforms);
- a **WebAssembly plug-in** (safest sandbox, most work).
The design above (signed key, signed package, registry, gates in few places) holds for all three; only step 5 changes.

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
