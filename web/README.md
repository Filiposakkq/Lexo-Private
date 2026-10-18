# Aside website

Everything of the website lives in this folder: the pages, and the small server behind their forms (`worker/`). The server is one Cloudflare Worker with its own D1 database, **separate from the licence server** (`../server/`): no shared code, database or secrets.

## What is where

| Path | What |
|---|---|
| `index.html`, `docs.html`, `releases.html`, `404.html` | pages written by hand |
| `pages/*.html` | sources of the other pages (compare, blog, developers, beta, showcase, roadmap, contact, feedback, report); `python tools/build_pages.py` turns them into `web/<name>.html` with the shared header (`pages/_head.html`) and footer (`pages/_foot.html`). Edit the sources, not the generated files. |
| `site.css`, `site.js` | shared styles; shared script: the menu with every page, the bottom bar on phones, the reading line, back to top, counters, and `AsideAPI` (the calls to the server, with Turnstile) |
| `data.js` | the numbers the pages show **when there is no server** (a page opened from disk, a preview): the beta round and the roadmap. With the server the pages read the same things from it, and you change them in the admin. |
| `img/`, `badge/`, `og.png`, `icon-*.png`, `favicon.svg` | pictures |
| `worker/` | the server: `src/` (public API, admin API, admin page, mails, daily job), `migrations/` (the database), `test/` |
| `.assetsignore` | keeps `worker/`, `pages/`, `tools/` and this file off the website |
| `tools/build_artifact.py` | a preview copy of the site (for a claude.ai artifact) |

Without the server every form still works by e-mail (it opens the visitor's mail app with the answers filled in); with it the forms send straight from the page.

## What the server does

- **Beta applications** (`beta.html`): saved, confirmed to the applicant by e-mail, announced to you; in the admin you accept, put on the waiting list or decline, with an e-mail. The free places are counted from the accepted applications.
- **Sponsored licences**: accepting starts the 30-day trial; the server mails the feedback form after 14 and 28 days (with one reminder 4 days later) and shows in the admin when both answers are in ("Ready for full licence"). The licence itself is still issued in the licence admin.
- **Bug reports** (`report.html#<code>`): accepted testers get a personal link; reports with a screenshot, their state visible to the tester.
- **Roadmap votes**: live counts, one vote per browser; cards are edited in the admin.
- **Showcase**: translations sent through a form, shown after you approve them.
- **Contact form**: messages in the admin; reply from the admin (sent from aside@filiposakkq.dev) or from your mail app.
- **Releases**: entered in the admin; the Releases page shows them, downloads go through `/api/download/<id>` and are counted.

Every e-mail goes from `Aside <aside@filiposakkq.dev>` with replies to the same address, so answers of people reach you through the existing forwarding.

## Set up (once, from your computer; the repository is not connected to Cloudflare)

You need Node 22. In a terminal:

```
cd web/worker
npm install
npx wrangler login                      # opens the browser, log in to Cloudflare
npx wrangler d1 create aside-web        # prints a database_id
```

1. Put the printed `database_id` into `worker/wrangler.toml` (replace the zeros).
2. Create the tables: `npm run db:init`
3. **Admin token**: `npx wrangler secret put ADMIN_TOKEN` and paste a long random string (40 characters is good). This is the password of `/admin`; keep it in your password manager.
4. **Resend**: in Resend, API Keys, Create API key, permission "Sending access", domain `filiposakkq.dev`. Then `npx wrangler secret put RESEND_API_KEY` and paste it. (A separate key from the licence server's, so either can be revoked alone.)
5. **Turnstile** (anti-bot, free): Cloudflare dashboard, Turnstile, Add widget, hostname `aside.filiposakkq.dev`, mode "Managed". Put the **site key** into `TURNSTILE_SITE_KEY` in `worker/wrangler.toml`, and the **secret key** with `npx wrangler secret put TURNSTILE_SECRET`.
6. **Deploy**: `npm run deploy` (uploads the pages of `web/` and the server).
7. **Address**: Cloudflare dashboard, Workers and Pages, `aside-web`, Settings, Domains and Routes, Add, Custom domain: `aside.filiposakkq.dev`.
8. Open `https://aside.filiposakkq.dev/admin` and sign in with the admin token. The overview warns when e-mail or Turnstile is not set up.

Optional but recommended: a second lock on the admin with **Cloudflare Access** (free): Zero Trust, Access, Applications, Add, Self-hosted, domain `aside.filiposakkq.dev`, path `admin*`, policy "Emails" with your address. You then log in with a code sent to your e-mail before the admin page opens.

## After a change

```
cd web
python tools/build_pages.py     # only when a file in pages/ changed
cd worker
npm test                        # the server's tests (no account or network needed)
npm run deploy
```

When a change adds a file to `worker/migrations/`, run `npm run db:init` before `npm run deploy`.

The daily job (feedback forms and reminders, clean-up) runs by itself at 08:17 UTC.

## Try it locally

```
cd web/worker
echo ADMIN_TOKEN=local-admin-token-0123456789 > .dev.vars
npx wrangler d1 migrations apply aside-web --local
npx wrangler dev --local        # http://localhost:8787 and http://localhost:8787/admin
```

Without `RESEND_API_KEY` nothing is mailed and without `TURNSTILE_SECRET` the forms are not checked; everything else works.

## Privacy

The server stores what people type into the forms, a hash of their address for the flood limits (removed after two days), and a hash of a random browser id for the votes. No cookies, no analytics. An application can be deleted in the admin (with its feedback and bug reports) when its author asks.

The texts and numbers of the pages come from docs/RUST-PORT.md, docs/GAME-TESTS.md, docs/FEATURES.md and docs/PRIVACY.md; update them when those change.
