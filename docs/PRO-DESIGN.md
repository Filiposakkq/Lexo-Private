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

## Limits, the free taste, and who enforces them
The limit of a feature (for example 20 lines per action) is **not a number in Free**. The pack says what it offers (`offers()` returns `{"limit": 20}` or `{"limit": None}`), and the host passes it on; the pack also **enforces** it inside its own code, so editing Free does not remove it.

**The demo must work without any extra step**, so it cannot be "download the pack first". Two kinds of Pro features, two ways of delivering them:

| Kind | Examples | Where it lives | Without a licence | With a licence |
|---|---|---|---|---|
| **Taste features**: cheap to copy, good for a first impression | batch machine translation, safe fixes with preview, regex replace, bulk categorisation | in the **demo pack**, which is **built into the installer** next to Free (closed source: only the built pack is shipped, never the source) | works with the limit the pack sets (20 lines per action) | the licence file switches the limit off |
| **Premium features**: the real value, hard to rebuild | carry a translation over to a new game version, fuzzy memory, LLM style check, release builder, team work | in the **premium pack**, **never shipped without a licence** | locked; the UI describes it and shows the value ("37 identical originals") | downloaded after the licence is seen |

So the user sees: install -> the taste features work with their limits at once -> a licence key lifts the limits and brings the premium features.

What this protects and what it does not (honest): the taste features are on every machine as a built pack, so a determined person could patch the pack, as with any shipped program; they are cheap, so that is accepted. The premium features are not on a machine without a licence at all. In the Python phase a shipped pack is readable Python and easy to change; the real protection of the taste pack comes with the compiled sidecar (Rust phase). Free itself holds neither feature nor limit in any phase.

(Until a feature is extracted its old limit is a number in Core; see FEATURES.md, column "In the Pro pack?".)

## Entering a licence (what happens, step by step)
Not built yet; this is the design.
1. The user types the **licence key** in Free (Settings -> Licence). Free contains only this small *activation client*: it sends the key to the licence server over HTTPS. It holds no secret and no feature.
2. The server checks the key and answers with (a) a **signed licence file** (who, which features, valid until, a device id if we bind one) and (b) the **address of the premium pack** to download, also signed.
3. Free downloads the premium pack and verifies its **signature** with the public key built into Free; a wrong or swapped pack is not loaded. It stores the licence file next to the packs.
4. At start each **pack** reads the licence file and verifies **its** signature with its own public key (offline, no network). A valid licence: it offers everything it contains. Missing, expired or broken: the demo pack offers its limits, the premium pack offers nothing.
5. **Putting a key into Free changes nothing by itself**: Free only fetches. What the user gets is decided by the packs and the signed file, which Free cannot forge (the private key stays with the author).

## The "30-day check" (optional online refresh)
A one-off licence can work completely offline forever. For a yearly licence, or to be able to revoke a leaked key, the pack can **ask the server every 30 days** whether the licence is still valid and fetch a renewed signed file. Rules that keep it fair: it never blocks on a failed check (no internet is normal); after a missed check the pack keeps working for a **grace period** (for example another 30 days) and then falls back to the demo limits, with a clear message; the user's translations and files are never touched. This is a choice for the owner (offline forever vs. yearly with a refresh), not a technical limit; the host and the interface do not change either way, because the check lives in the pack.

## Updates (the question "one app, but two things")
- Free and the pack have **separate version numbers**, and both know the **interface version** (`API` in `extensions.py`, now 1).
- The application's updater looks at one **manifest**: `{free: version, pack: version, api: 1, files: [url, sha256, signature]}`.
- A pack with a different interface version is **not loaded** (the user is told, nothing crashes): a new Free that needs interface 2 will download the matching pack, an old Free keeps working with its old pack.
- The pack is **signed**; Free checks the signature with a public key built into it before it loads the pack (`ExtensionHost(trust=...)` is the hook, a function that may veto a pack). A swapped or damaged pack is ignored.
- The licence is a signed file; it is checked **inside the pack** (which decides what to offer: everything, a demo with limits, nothing). The pack is downloaded from a server after the licence is seen; the details belong to the licence server (roadmap step 6).

## What is done and what is not
Done (this change):
- `core/lexo/extensions.py`: the host, the interface (version 1), loading by name, the `trust` hook, plain-JSON calls, interface texts of the pack.
- `pro/lexo_pro/`: the first pack, offering `publish.web`. Its code (`publish.py`, the git work) moved out of Core. Its Czech texts are in the pack (`pro/lexo_pro/ui/cs.json`), not in Core's language pack.
- The feature `publish.web` is **extracted**: without the pack it is locked whatever the flags say; a test proves it. A test proves that Core and Free never import the pack, and that `core/lexo/publish.py` is gone.
- `py run_tests.py pro` (the pack's own tests, only in the private repository).

Not done (each is its own piece of work, listed so nothing is forgotten):
- **Four features are still in Core** behind the old flag (`Feature.extracted = False`): `mt.batch`, `fixes.safe`, `replace.regex`, `bulk.categorize`. Until each is extracted, someone who edits Core can unlock it. Each needs its own split, because the module holds both the free and the paid half (a single line of machine translation is free, a batch is Pro; the safe fixes with a preview are Pro, the plain speaker fix is free; plain replace is free, regex replace is Pro; approving identical lines is free, suggesting groups is Pro).
- The **sidecar transport**, the **signature check** and the **manifest updater**.
- The **licence**: the file, the check inside the pack, the demo limits, the server that hands out the pack.
- Decisions that belong to the owner: price and form of the licence, and the licence of the public part (see FEATURES.md, open questions).

## For a developer
- Running from the repository: `py free/run_lexo.py` puts `pro/` on the import path when the folder exists, so the pack is there. `LEXO_NO_PRO=1` starts the application as a Free installation would be.
- A new Pro feature: add it to `features.CATALOG` with `extracted=True`; write the implementation in `pro/lexo_pro/`; offer it in `Provider.offers()`; call it from the app with `self.ext.call(feature, method, payload)`; gate it with `self._gate(feature)`; put its tests in `pro/tests/`; put the Czech texts of its messages in `pro/lexo_pro/ui/cs.json`.
- Messages written in the pack are returned with `messages.wire(...)` so that they stay translatable JSON.
