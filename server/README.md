# Aside licence server

A Cloudflare Worker with a D1 database (private, never part of the public repository). Design: [`docs/PRO-DESIGN.md`](../docs/PRO-DESIGN.md), "The licence server".

## What it does

| Call | Who | What |
|---|---|---|
| `POST /v1/activate {key, device}` | the application | takes a place for the device (2 per licence) and returns the signed licence file |
| `POST /v1/refresh {licence, device}` | the application | the regular check (every `check_days`, default 14); a renewed file, or a refusal for a revoked licence |
| `POST /v1/deactivate {licence, device}` | the application | frees the place of the device |
| `POST /v1/webhook/lemonsqueezy` | the payment provider | a paid order makes a licence (once per order) and mails the key; a refund revokes it |
| `GET /admin`, `/admin/api/...` | the owner | the admin page (see "The admin page" below) |
| `GET /v1/health` | anyone | `{"ok": true}` |

The licence file is the one the Pro pack checks (`pro/rust/lexo-pro/src/licence.rs`): `{payload, sig}`, Ed25519 over the bytes of the payload. Only a **hash** of the key is stored; the key is shown once (to the buyer by e-mail, to the owner in the admin page).

## Development (no Cloudflare account needed)

Node 22 and `npm install` in this folder (`.npmrc` has `legacy-peer-deps` because of a bug of npm 10 with the peer dependencies of the Cloudflare tools).

```
npm test            # the Worker in Miniflare with a real D1 and a throw-away key pair
npm run typecheck
node scripts/cross-check.mjs <lexo-pro-licence>   # a file signed here is accepted by the Rust verifier
```

## First deployment (the owner)

1. `npx wrangler login`
2. `npx wrangler d1 create aside-licence`, put the printed `database_id` into `wrangler.toml`.
3. `npx wrangler d1 migrations apply aside-licence --remote`
4. Secrets (each asks for the value; they are never in a file or in git):
   - `npx wrangler secret put LICENCE_SIGNING_KEY`: the content of `licence-private.key` (64 hex digits) from `lexo-pro-licence keygen`
   - `npx wrangler secret put ADMIN_TOKEN`: a long random string (at least 16 characters; 40 is better) for the admin page
   - `npx wrangler secret put WEBHOOK_SECRET`: the signing secret you choose when you create the webhook at the payment provider
   - `npx wrangler secret put RESEND_API_KEY`: optional, to mail the key (set up Resend with your domain first); without it the key of a purchase is not mailed and is re-issued from the admin page ("New key")
5. `npx wrangler deploy`, then give the Worker the address `api.aside.filiposakkq.dev` (Cloudflare dashboard: Workers, the Worker, Settings, Domains and Routes).
6. At the payment provider create a webhook: address `https://api.aside.filiposakkq.dev/v1/webhook/lemonsqueezy`, events `order_created` and `order_refunded`, and the signing secret from step 4. (The shape of the events is written from the documentation of Lemon Squeezy; check it with a test order. Another provider needs only another function like `parseLemonSqueezy` in `src/handlers.ts`.)
7. In the Cloudflare dashboard add rate limiting rules for `/v1/*` and `/admin/api/*` (a few requests per minute per address is plenty for people).

The admin page is `https://api.aside.filiposakkq.dev/admin`: type the admin token, it stays only in that browser tab. For a second lock put Cloudflare Access in front of `/admin*` (free for a few users).

## What is not here yet

- The activation client in the application (Settings, Licence) and the regular check in the Pro pack.
- The download of the premium pack (encrypted per licence, with a watermark) and the update channel.
- Upgrade discounts for a new major version.

## Automatic deployment

`.github/workflows/server-deploy.yml` deploys the Worker after a merge to `main` that changes `server/`, once the typecheck and the tests pass. It needs two secrets in GitHub (Settings, Secrets and variables, Actions): `CLOUDFLARE_API_TOKEN` (create it in the Cloudflare dashboard: My Profile, API Tokens, Create Token, the template "Edit Cloudflare Workers"; limit it to this account) and `CLOUDFLARE_ACCOUNT_ID`. Without the token the deploy step is skipped. The secrets of the Worker itself stay in Cloudflare. Database changes (`migrations/`) are not applied by the deploy: run the new SQL in the D1 console (or `wrangler d1 migrations apply`) before merging code that needs it.

## Changes of the database (do this BEFORE merging code that needs it)

The deploy does not touch the database. When a change adds a file to `migrations/`, run its SQL in the D1 console (Cloudflare, Storage and databases, D1, `aside-licence`, Console) first, and only then merge the code. Executed so far by hand: `0001_init.sql`, then `0002_devices.sql` (the name and the version of a computer).

## Rotating the signing key

Every licence file names the key that signed it (`kid` in the payload; a file without one was signed by `k1`). The pack trusts a list of keys (`TRUSTED_KEYS` in `pro/rust/lexo-pro/src/licence.rs`), the server signs with one (`LICENCE_KID`, default `k1`) and verifies files of older keys (`LICENCE_OLD_PUBLIC_KEYS`). Do this **before** the first release if you can: the first builds then already know the list.

**Planned rotation** (no leak):
1. `lexo-pro-licence keygen <folder>` makes a new pair; keep the private key outside the repository.
2. Add `("k2", "<new public hex>")` to `TRUSTED_KEYS`, release a pack version, wait until it has spread (the licence check renews files only when it is due, so allow a few weeks).
3. In Cloudflare: `LICENCE_SIGNING_KEY` = the new private key; in `wrangler.toml`: `LICENCE_KID = "k2"`, `LICENCE_PUBLIC_KEY` = the new public key, `LICENCE_OLD_PUBLIC_KEYS = '{"k1": "<old public hex>"}'`. Deploy. From now on every renewed file is signed by `k2`; files of `k1` keep working until they are renewed.
4. Later, remove `k1` from the pack and from `LICENCE_OLD_PUBLIC_KEYS`.

**After a leak of the private key:** as above, and in the same pack release put the old id into `DISTRUSTED_KEYS` (files signed by it are refused, with the reason "no longer trusted"). The server keeps renewing them while `LICENCE_OLD_PUBLIC_KEYS` lists the old key, so honest users get a new file at their next check; drop the old key from that list once the licence ids in the database are checked, because with it the server accepts a forged file of a real licence id. Users whose pack is not yet updated see the licence refused until they update.

## Limits of a licence

- **Computers:** 2 per licence (`max_devices`, the owner can change it per licence).
- **Freeing a computer by the user** (in the application: another computer, or "Remove from this computer"): **2 times for the whole life of the licence** (`RELEASE_LIMIT` in `src/handlers.ts`). The owner frees computers in the admin without a limit and those do not count.
- **Check:** the licence file is renewed every 14 days (`check_days`), and there is the same time of grace without a connection.

## A lost key (self-service)

The key is stored only as a hash, so it cannot be sent again; the user gets a **new** key and the old one stops working. To make sure a stranger cannot do that to someone, it takes two steps (`src/recovery.ts`):
1. `POST /v1/recover/request {email}` mails a 8-character code to the address of the purchase. It answers `ok` for every address (no way to find out who is a customer). Nothing changes yet.
2. `POST /v1/recover/confirm {email, code}` with the right code replaces the key; the new key is returned and mailed. Computers that already have a licence file keep working.

Limits: **1 code per hour and 3 per day** for an address (also counted for unknown addresses), the code is valid **15 minutes** and has **5 attempts**, and a licence can be recovered this way **once**; the next time goes through the owner (admin: "New key"). If an address has several licences, the newest active one is used.

**Database change before the code is deployed** (D1 console; it is `migrations/0003_recovery.sql`): create the tables `recovery_requests` and `recovery_codes`.

## Limits per network address

The calls of the application are limited per address and hour in the code (`src/ratelimit.ts`; the free plan of Cloudflare allows only one rule of its own): `recover/request` 5, `recover/confirm` 20, `activate` 30, `deactivate` 30, `release` 30, `devices` 60, `refresh` 120. The address is taken from `CF-Connecting-IP` (without it, in a local run and in the tests, nothing is limited), hashed together with the day, counted in the table `rate_limits` and deleted after a few hours. **Database change before the code is deployed:** `migrations/0004_ratelimit.sql` in the D1 console.
## What the licence does and does not stop (hardening)

- The age of a licence file is measured with the later of the clock, the highest time the computer has seen (`clock.seen` next to the licence) and the signed time of issue, so turning the clock back does not keep a cancelled licence alive. Someone who deletes `clock.seen` gets back at most the time to the end of the file (14 + 14 days).
- The device id includes the identifier of the machine, so copying `licence.json` and `device.id` to another computer gives another device that has to take a place. (After this change, computers activated before it need to enter the key once more.)
- A place is taken in one statement that checks the limit, so parallel activations cannot exceed `max_devices`.
- Not stopped: patching the program, a replaced Core (it is open source), a licence file kept offline until it ends. Rate limits by IP belong to Cloudflare (a rule on `/v1/activate`, `/v1/refresh`, `/v1/recover/request`).

## The admin page

`GET /admin` (the page; in front of it Cloudflare Access and the token `ADMIN_TOKEN`) and `/admin/api/...` (every call needs `Authorization: Bearer <ADMIN_TOKEN>`). Dark red, no libraries, no dialogs of the browser.

**What it shows:** the numbers (active licences, computers in use, overdue checks, revoked, lost keys replaced, refused calls in 24 h); the list of licences with badges (key replaced, no releases left, shared computer id) and the reason of a revoking; the panels *Needs a look* (computers over the limit three times or more, the key of a revoked licence used, the limit of releases reached, a computer id on two licences, a lost key replaced), *Not activated yet* (bought more than 7 days ago), *Recent activity*, *Versions in use* (of the computers that hold a place) and *System* (mail service set up or not, when a key was last mailed, the signing key id and the trusted ones, payment hook, size of the database, how often the limits per address hit, a button to reset them). It never shows a secret, only whether it is set.

**Search** (the box above the list) looks at the licence id, the e-mail, the note and the notes of the history, and the name and the id of the computers.

**What the owner can do on a licence:** copy the id; computers allowed; check interval; edit the contact; **send the key again** (the key is only a hash, so a new key is made and mailed); **new key** (optionally mailed, optionally *also free all computers* after a leak); **revoke** with a kind (refund, abuse, test, other) and a reason, restore; erase personal data (also removes the notes); free a single computer; **notes** (a history, newest first; never shown to the buyer); the id of a computer is copied by a click and shows where else it is.

| Call | What |
|---|---|
| `GET overview?q=&events=` | the list (with `user_releases`, `recovered`, `revoke_reason`, `shared_devices`), the log, the numbers |
| `GET insights` | the panels beside the list (attention, not activated, versions, limits, health); loaded with the page, not at every letter of the search |
| `GET licence/<id>` | the detail: computers (with `also_on`), events, `notes`, `user_releases`, `release_limit`, `recovered` |
| `POST licence/<id>/note {text}`, `note-delete {id}` | notes (up to 500 characters) |
| `POST licence/<id>/revoke {category, reason}` | the log keeps `<category>: <reason>` |
| `POST licence/<id>/rekey {send, free_all}` | a new key; `free_all` frees every computer |
| `POST test-mail {to}` | a test e-mail; answers `sent` and `reason` (`not_configured`, or what Resend said) |
| `POST limits/reset` | deletes the counters of the limits per address |

The version of the server shown on the page is `SERVER_VERSION` in `src/admin.ts` (change it with a release). No database change comes with these functions: notes, revoke reasons and limit hits are rows of `events` (`note`, `revoked`, `rate-limited`).
