# Pro: how it is kept apart, delivered and updated

Decided with the owner. Plain words first, details after. Written in English like the other docs; the owner reads it in Czech through the chat.

## The problem
Free is public. If a Pro feature were in the public code behind a flag (`pro = True`, a limit of 20), anybody could change the flag. So:

1. **Pro code is not in the public repository and not in the Free download.** It lives only in `pro/` of this private repository and is delivered separately.
2. **Free contains only a socket**, never the feature: the catalog of gated features (`core/lexo/features.py`), the extension host (`core/lexo/extensions.py`), the places where the UI offers a feature.
3. **The limits, the demo mode and the licence check are inside the Pro pack.** Free has no flag that means "Pro" and no number that means "20 items".

Nothing is unbreakable: someone who has a leaked Pro pack can use it. The aim is that a casual change of Free's code gives nothing, and that a licensed user gets a simple, fair experience.

## What the user sees: one application, one more file
A user installs Lexo (Free). With a licence, the application downloads the **Pro pack**, a second package that sits next to it. The user sees one application whose menus show more features. Without the pack the same menus show the features as locked, with a short description.

## How Free and the pack talk (the "interface")
A call is: a **feature id** (`publish.web`), a **method** (`plan`) and a **dict that can be written as JSON**. The answer is such a dict. An error is a message for the user. Nothing else crosses the border: no live Python objects, no shared state. (Today `core/lexo/extensions.py` enforces it: every payload goes through JSON and back.)

A Pro feature **computes and returns data** (a plan, proposals, translations). The single writer of `.rpy` files stays in Core. (Publishing, which pushes to a git repository, is the one feature that acts on its own, and it asks for a typed confirmation.)

Because the border is plain JSON it does not matter how the pack runs:

| Form | What it is | When |
|---|---|---|
| **In-process module** | the pack is a Python package `lexo_pro` that Core imports by name at run time | today (the Python app) and for tests |
| **Sidecar** | the pack is a separate small program that Free starts and talks to on its standard input/output (JSON lines) | with the Rust / Tauri app (Tauri has sidecars built in) |

Switching from the first to the second changes only the transport inside `ExtensionHost`; no feature code changes. **"Sidecar" is just that: a second program next to the app, started and stopped by the app.** The user does not run it by hand.

## The Rust pack and its sidecar (built)
The pack exists twice: `pro/lexo_pro` (Python, used by the Python application) and `pro/rust/lexo-pro` (Rust, for the Tauri application). Both give the same answers; `pro/golden/pack.json` holds 110 payloads with the answers of the Python pack and the Rust tests compare against it (`py pro/tests/golden_pro.py` rewrites it; a Python test fails when it is out of date).

The Rust pack is a **program**, `lexo-pro-sidecar` (`pro/rust/lexo-pro`). The application finds it through the variable `LEXO_PRO_SIDECAR` or in a `pack/` folder next to the application, starts it and speaks one JSON line per request on its standard input and output (`SidecarProvider` in `core/lexo`'s Rust port, `rust/crates/lexo-core/src/extensions.rs`):

| request | answer |
|---|---|
| `{"id": 1, "method": "hello"}` | `{"id": 1, "ok": true, "result": {"api": 1, "version": "0.2.0"}}` |
| `{"id": 2, "method": "offers"}` | the features it offers now, each with its limit (`null` = none) |
| `{"id": 3, "method": "ui_strings", "code": "cs"}` | the texts of the messages the pack writes, in that language |
| `{"id": 4, "method": "call", "feature": "fixes.safe", "m": "plan", "payload": {...}}` | `{"id": 4, "ok": true, "result": {...}}` |

An error is `{"id": N, "ok": false, "error": <text or {"$m": ...}>}`. A pack that cannot be started is a note in the settings, never an error; `LEXO_NO_PRO` leaves it out (what a Free installation looks like).

**The licence check is built in the pack** (`pro/rust/lexo-pro/src/licence.rs`): a signed file `licence.json` (Ed25519, checked offline), states licensed / grace / expired / refused / demo, the check interval inside the file, binding to a device id and to the major version, the demo limits otherwise; the tool `lexo-pro-licence` (`keygen`, `issue`, `show`, `device`) makes key pairs and licence files, so licences can be issued by hand before the server exists. **Still open:** the public key of the owner is built in (`PUBLIC_KEY_HEX`); the pack is still built with the feature `dev-unlock` (without any file it behaves as licensed; the release build uses `--no-default-features`); the default interval of the online check is 14 days; the activation client in the application and the server. `LEXO_PRO_DEMO=1` shows the demo limits. The network client (`lexo_core::net::post`) is built; the pack uses it for the batch machine translation.

## Limits, the free taste, and who enforces them
The limit of a feature (for example 20 lines per action) is **not a number in Free**. The pack says what it offers (`offers()` returns `{"limit": 20}` or `{"limit": None}`), and the host passes it on; the pack also **enforces** it inside its own code, so editing Free does not remove it.

**The demo must work without any extra step**, so it cannot be "download the pack first". Two kinds of Pro features, two ways of delivering them:

| Kind | Examples | Where it lives | Without a licence | With a licence |
|---|---|---|---|---|
| **Taste features**: cheap to copy, good for a first impression | batch machine translation, safe fixes with preview, regex replace, bulk categorisation | in the **demo pack**, which is **built into the installer** next to Free (closed source: only the built pack is shipped, never the source) | works with the limit the pack sets (20 lines per action) | the licence file switches the limit off |
| **Premium features**: the real value, hard to rebuild | carry a translation over to a new game version, fuzzy memory, LLM style check, release builder, team work | in the **premium pack**, **never shipped without a licence** | locked; the UI describes it and shows the value ("37 identical originals") | downloaded after the licence is seen |

So the user sees: install -> the taste features work with their limits at once -> a licence key lifts the limits and brings the premium features.

What this protects and what it does not (honest): the taste features are on every machine as a built pack, so a determined person could patch the pack, as with any shipped program; they are cheap, so that is accepted. The premium features are not on a machine without a licence at all. In the Python phase a shipped pack is readable Python and easy to change; the real protection of the taste pack comes with the compiled sidecar (Rust phase). Free itself holds neither feature nor limit in any phase.


## Entering a licence (what happens, step by step)
Built: the key box and the state in Settings, Licence (`38-pro.js`), the routes `/api/licence`, `/api/licence/activate`, `/refresh`, `/deactivate` (application, not part of the Core contract) and `pro/rust/lexo-pro/src/licence_client.rs`; the pack renews the file quietly at its start when the check is due within 3 days. Not built: the download of the premium pack (steps 3 and the encryption); until then the sidecar of the Pro installer holds all features and the signed file unlocks them.
1. The user types the **licence key** in Free (Settings -> Licence). Free contains only this small *activation client*: it sends the key to the licence server over HTTPS. It holds no secret and no feature.
2. The server checks the key and answers with (a) a **signed licence file** (who, which features, valid until, a device id if we bind one) and (b) the **address of the premium pack** to download, also signed.
3. Free downloads the premium pack and verifies its **signature** with the public key built into Free; a wrong or swapped pack is not loaded. It stores the licence file next to the packs.
4. At start each **pack** reads the licence file and verifies **its** signature with its own public key (offline, no network). A valid licence: it offers everything it contains. Missing, expired or broken: the demo pack offers its limits, the premium pack offers nothing.
5. **Putting a key into Free changes nothing by itself**: Free only fetches. What the user gets is decided by the packs and the signed file, which Free cannot forge (the private key stays with the author).

## Sharing the pack and the licence: what is done about it
Nobody can fully stop a licensed person from sending files on; the aim is that sharing is inconvenient, traceable and not worth it. Decided with the owner:

1. **The pack alone unlocks nothing.** The premium pack offers nothing without a valid, signed licence file.
2. **A licence is bound to devices: 2 per licence.** The licence file carries a device id and the server counts activations. A copied file does not work on a third computer. The user frees a device (a button in Settings, which tells the server) when changing computers.
3. **Every download is made for one licence.** The server **encrypts** the premium pack with a key that belongs only to that licence (the key arrives with the signed licence file and lives in memory only while the application runs), and puts an invisible **watermark** into it (the licence id, in several places of the pack). A leaked pack is useless without its key and can be traced to the licence it was made for.
4. **Online check, now and then.** The pack asks the server whether the licence is still valid and receives a renewed signed licence file. The interval is a value **inside the signed licence file** (default **30 days**; **14 days** is possible for a licence that is suspected of being shared), so it can be changed without updating the application. A failed check never blocks work (no internet is normal): after a missed check the pack keeps working for a grace period of the same length, then falls back to the demo limits with a clear message. Translations and project files are never touched. A revoked licence is simply not renewed.
5. The value of Pro is in what cannot be copied from a screenshot (carry-over, fuzzy memory, style check, release builder); the cheap taste features are in the installer's demo pack and are not what the income stands on.

What it does not prevent: a licensed person who is determined to patch the program, or to share a whole computer. The price is meant to be low enough that buying is simpler than working around it.

## Decisions of the owner
- Watermark and per-licence encryption of the premium pack: **yes**.
- Online check every **30 days** (14 for suspicious licences), grace period of the same length: **yes**.
- **2 devices** per licence: **yes**.
- **One-off payment: decided.** A one-off licence for the current major version (1.x, updates and fixes included), a paid upgrade for the next major version. Why: no running cost per user (the user brings their own machine translation key, nothing is hosted), the target users are hobby translators who dislike subscriptions, and a leaked licence can still be revoked by the online check, since a revocation needs no expiry date. A yearly licence would make sense only if the product grew a running cost (hosted translation, a team server); the code would not change, only the contents of the licence file (what it covers and until when).

## The licence server (built in `server/`: Worker, D1 schema, admin page, tests; see `server/README.md`. Not deployed yet; the payment provider and the activation client in the application are still open)
A small service on Cloudflare. Nothing of it is in the public repository (it lives in a private folder, e.g. `server/`). Deployment, the payment account and the domain are the owner's; the code, the database schema and the tests are written here.

**Parts**
| Part | Used for |
|---|---|
| Workers | the four calls below and the payment webhook |
| D1 (SQL) | tables: `licences` (id, key hash, buyer, major version, check interval, state), `activations` (licence, device id, dates), `downloads` (licence, pack version, time: to trace a leaked pack), `revocations` |
| R2 (file storage) | the packs: demo (also offered for download) and premium (encrypted per download) |
| Secrets | the private **signing key** (Ed25519) and the **master encryption key**. Never in a repository or in the application; the application knows only the public key |
| A payment provider that acts as the seller of record (Paddle or Lemon Squeezy; check conditions and fees before choosing) | takes the one-off payment and handles VAT; on payment it calls the webhook, which creates a licence key and mails it |

**The four calls** (HTTPS, JSON; the application talks to nothing else)
1. `activate {key, device id}`: checks the key, counts the activations (at most 2), returns the **signed licence file** (licence id, features, covered major version, device id, issued, check interval, the wrapped pack key) and the address of the premium pack.
2. `pack`: sends the premium pack for that licence. The Worker **encrypts** it with a key derived from the master key and the licence id (HKDF; nothing per-licence is stored) and writes the **watermark** (the licence id) into reserved slots of the file before encrypting; the download is logged in `downloads`.
3. `refresh {licence, device id}`: the regular check (30 days by default; the interval is inside the signed file). Returns a renewed signed file, or refuses for a revoked licence.
4. `deactivate {licence, device id}`: frees a device.

**Rules that keep it safe and fair**
- The application never needs the server to run: the signed file is verified offline with the public key. A failed `refresh` only starts the grace period.
- The device id is a hash of an id stored by the application and a machine id, never anything readable (no names, no paths, no project data are ever sent).
- The server stores the least it can: a hash of the key, the buyer's e-mail from the payment provider, device hashes, dates.
- Rate limits on every call, and the keys are long random strings, so guessing is pointless.
- A leaked pack is traced through its watermark to a licence, which can then be revoked (it is simply not renewed at the next `refresh`).
- Changing the interval, the number of devices or the covered version is a change in the file the server signs; the application does not change.

**Order of work**: schema and the four calls with local tests (Miniflare), then the signing and the pack encryption, then the webhook with the payment provider's test mode, then the client side (the activation client in Free, the licence check inside the pack), then a dry run of the whole path with a test licence. Each step is separately checkable.

## Updates (the question "one app, but two things")
- Free and the pack have **separate version numbers**, and both know the **interface version** (`API` in `extensions.py`, now 1).
- The application's updater looks at one **manifest**: `{free: version, pack: version, api: 1, files: [url, sha256, signature]}`.
- A pack with a different interface version is **not loaded** (the user is told, nothing crashes): a new Free that needs interface 2 will download the matching pack, an old Free keeps working with its old pack.
- The pack is **signed**; Free checks the signature with a public key built into it before it loads the pack (`ExtensionHost(trust=...)` is the hook, a function that may veto a pack). A swapped or damaged pack is ignored.
- The licence is a signed file; it is checked **inside the pack** (which decides what to offer: everything, a demo with limits, nothing). The pack is downloaded from a server after the licence is seen; the details belong to the licence server (roadmap step 6).

## What is done and what is not
Done:
- `core/lexo/extensions.py`: the host, the interface (version 1), loading by name, the `trust` hook, plain-JSON calls, interface texts of the pack.
- `pro/lexo_pro/`: the pack. **All five gated features are in it and only in it**: `publish.web` (`publish.py`), `bulk.categorize` (`categorize.py`), `replace.regex` (`regex_replace.py`), `mt.batch` (`mt_batch.py`), `fixes.safe` (`fix_safe.py`).
  Each works on plain data: Core finds the candidate lines and gathers what is needed, the pack decides (a plan, a list of proposals, translations, a substitution) and returns it, Core shows it and writes what the translator confirms. The single writer of `.rpy` files stays in Core.
- The limits of the demo mode live in the pack (`lexo_pro/licence.py`: 20 items per action for batch translation and safe fixes, 5 files for bulk categorisation (it works on files, which are few in a small game), no demo for regex replace and publishing) and are enforced there; Core has no limit, no flag and no `Entitlements` any more.
- Without the pack every gated feature is locked in every place it is used (a test lists them). The free halves keep working: plain find & replace, one line of machine translation, applying a chosen group assignment, the validation and the spelling hints.
- Tests: Core and Free never import the pack, and the code of the Pro features (the planners, the regex substitution, the publisher) is not in the public packages (`free/tests/test_pro_boundary.py`); the pack has its own tests (`py run_tests.py pro`, 67). The UI self-test runs with the pack (monorepo) and without it (the Pro steps then check the lock).

Not done (each is its own piece of work, listed so nothing is forgotten):
- The **sidecar transport**, the **signature check** and the **manifest updater**.
- The **licence**: the file, the check inside the pack (`licence.licensed()` is a development stub: licensed unless `LEXO_PRO_DEMO=1`), the server that hands out the pack, the split of the pack into the demo pack in the installer and the premium pack downloaded after a licence.
- The shapes of the answers of the Pro operations are not recorded in `core/contract/shapes.json` (Core's contract test runs without the pack); the pack tests check `ok` and plain JSON only.
- A demo limit is "items per action"; nothing ties the actions together, so a client that repeats the call gets more (accepted for the cheap taste features).
- Decisions that belong to the owner and are still open: the licence of the public part (see FEATURES.md, open questions) and the price.

## The UI of the Pro features
For now the screens of the five features are part of the Free UI, but each one is entered through `38-pro.js`, which asks Core whether the feature is available and says "Pro feature" when it is not (variant A, possibly temporary). Variant B, where the pack carries its own screens, is open until after the port (docs/RUST-PORT.md, section 8).

## After the port to Rust
How the pack connects to the Rust Core, the options for running it without Python (a closed Rust crate that depends on Core is recommended) and what has to be recorded in golden data first: docs/RUST-PORT.md, section 9.

## For a developer
- Running from the repository: `py free/run_lexo.py` puts `pro/` on the import path when the folder exists, so the pack is there. `LEXO_NO_PRO=1` starts the application as a Free installation would be.
- A new Pro feature: add it to `features.CATALOG`; write the implementation in `pro/lexo_pro/` on plain data; offer it in `Provider.offers()` (limits in `licence.py`); call it from the app with `self._ext_call(feature, method, payload)` (it gates, calls and turns a refusal into a message); put its tests in `pro/tests/` (base class `ProCase`); put the Czech texts of its messages in `pro/lexo_pro/ui/cs.json`.
- Messages written in the pack are returned with `messages.wire(...)` so that they stay translatable JSON.
