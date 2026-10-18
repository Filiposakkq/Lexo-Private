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

- **The admin** (`/admin`): behind Cloudflare Access (your e-mail and a code), optionally also an emergency token; works on a phone.
- **Beta applications** (`beta.html`): saved, confirmed to the applicant by e-mail, announced to you; in the admin you accept, put on the waiting list or decline, with an e-mail. The free places are counted from the accepted applications.
- **Sponsored licences**: accepting starts the 30-day trial; the server mails the feedback form after 14 and 28 days (with one reminder 4 days later) and shows in the admin when both answers are in ("Ready for full licence"). The licence itself is still issued in the licence admin.
- **Bug reports** (`report.html#<code>`): accepted testers get a personal link; reports with a screenshot, their state visible to the tester.
- **Roadmap votes**: live counts, one vote per browser; cards are edited in the admin.
- **Showcase**: translations sent through a form, shown after you approve them.
- **Contact form**: messages in the admin; reply from the admin (sent from aside@filiposakkq.dev) or from your mail app.
- **Releases**: entered in the admin; the Releases page shows them, downloads go through `/api/download/<id>` and are counted.

Every e-mail goes from `Aside <aside@filiposakkq.dev>` with replies to the same address, so answers of people reach you through the existing forwarding.

## Set up (once; everything works in a browser, also on a phone)

The website deploys itself through GitHub Actions (`.github/workflows/web-deploy.yml`) with the same two GitHub secrets as the licence server (`CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`). The Cloudflare project is not connected to the repository; GitHub only uses the token.

**1. Cloudflare: database and token**
1. Workers and Pages, D1, Create database, name `aside-web`. Copy its **Database ID** into `database_id` in `worker/wrangler.toml` (not a secret). Done: `10e68128-…`.
2. My Profile, API Tokens: edit the token the licence server uses and add the permission **Account, D1, Edit** (the deploy creates the tables).

**2. GitHub: the first deploy**
3. Merge the change with the database id into `main`. The workflow runs the tests, creates the tables and deploys the Worker `aside-web` (Actions tab, "Deploy website").

**3. Cloudflare: keys and address**
4. Resend: API Keys, Create API key, "Sending access", domain `filiposakkq.dev`.
5. Turnstile, Add widget, hostname `aside.filiposakkq.dev`, mode Managed. The **site key** goes into `TURNSTILE_SITE_KEY` in `worker/wrangler.toml` (public); keep the secret key for the next step.
6. Workers and Pages, `aside-web`, Settings, Variables and Secrets, Add, type **Secret**:
   - `HASH_SALT`: a long random string; set it once and never change it (it anonymises addresses and votes)
   - `RESEND_API_KEY`: the key from step 4
   - `TURNSTILE_SECRET`: the secret key from step 5
   - `ADMIN_TOKEN` (optional): an emergency password for the admin, at least 16 characters; leave it out and only Cloudflare Access opens the admin

   A deploy never removes these.
7. Settings, Domains and Routes, Add, Custom domain: `aside.filiposakkq.dev` (disconnect it first if something else uses that address).

**4. Cloudflare Access: the lock of the admin**
8. Zero Trust (the first time it asks for a team name and the Free plan), Access, Applications, Add an application, Self-hosted:
   - application domain `aside.filiposakkq.dev`, path `admin`
   - policy: Allow, Include, Emails, your address
   - session duration as you like (for example 1 month)
9. From the application's overview copy the **Application Audience (AUD) Tag**, and your team domain (`<team>.cloudflareaccess.com`, Zero Trust, Settings, Custom Pages). Put them into `ACCESS_AUD` and `ACCESS_TEAM_DOMAIN` in `worker/wrangler.toml` and merge.
10. Open `https://aside.filiposakkq.dev/admin`: Cloudflare asks for your e-mail, sends a code, and the admin opens. The server checks the Access signature itself, so if Access were ever switched off by mistake the admin stays locked.

The overview of the admin warns when e-mail or Turnstile is not set up.

## After a change

A merge to `main` that changes `web/` deploys by itself. A pull request runs the checks only. To deploy again by hand: Actions, Deploy website, Run workflow (works in the GitHub app).

The checks also fail when a file in `pages/` changed but `python tools/build_pages.py` was not run; run it and commit the generated pages. New tables (a new file in `worker/migrations/`) are created by the deploy.

The daily job (feedback forms and reminders, clean-up) runs by itself at 08:17 UTC.

### Deploying from a computer instead

```
cd web/worker
npm install
npx wrangler login
npm run db:init                 # the tables
npm test && npm run deploy
```

## Try it locally

```
cd web/worker
printf 'ADMIN_TOKEN=local-admin-token-0123456789\nHASH_SALT=local\n' > .dev.vars
npx wrangler d1 migrations apply aside-web --local
npx wrangler dev --local        # http://localhost:8787 and http://localhost:8787/admin
```

Without `RESEND_API_KEY` nothing is mailed and without `TURNSTILE_SECRET` the forms are not checked; everything else works.

## Privacy

The server stores what people type into the forms, a salted hash of their address for the flood limits (removed after two days), and a salted hash of a random browser id for the votes (the salt is `HASH_SALT`, known only to the server). No cookies, no analytics. An application can be deleted in the admin (with its feedback and bug reports) when its author asks.

The texts and numbers of the pages come from docs/RUST-PORT.md, docs/GAME-TESTS.md, docs/FEATURES.md and docs/PRIVACY.md; update them when those change.
