# Aside licence server

A Cloudflare Worker with a D1 database (private, never part of the public repository). Design: [`docs/PRO-DESIGN.md`](../docs/PRO-DESIGN.md), "The licence server".

## What it does

| Call | Who | What |
|---|---|---|
| `POST /v1/activate {key, device}` | the application | takes a place for the device (2 per licence) and returns the signed licence file |
| `POST /v1/refresh {licence, device}` | the application | the regular check (every `check_days`, default 14); a renewed file, or a refusal for a revoked licence |
| `POST /v1/deactivate {licence, device}` | the application | frees the place of the device |
| `POST /v1/webhook/lemonsqueezy` | the payment provider | a paid order makes a licence (once per order) and mails the key; a refund revokes it |
| `GET /admin`, `/admin/api/...` | the owner | the admin page: licences, devices, events, issue / revoke / restore / new key / free a device / check interval |
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
