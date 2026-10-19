# Aside website

Everything of the website lives in this folder: the pages, and the small server behind their forms (`worker/`). The server is one Cloudflare Worker with its own D1 database, **separate from the licence server** (`../server/`): no shared code, database or secrets.

## What is where

```
web/
├── public/              the website, exactly as visitors get it (this folder is what gets deployed)
│   ├── index.html       home page            ┐
│   ├── docs.html        documentation        │ written by hand
│   ├── releases.html    releases, changelog  │
│   ├── 404.html         "page not found"     ┘
│   ├── beta.html, roadmap.html, … made from templates/ (do not edit, see below)
│   └── assets/          site.css, site.js, data.js, img/, badge/, icons, og.png
├── templates/           sources of the generated pages (+ _head.html, _foot.html shared by all of them)
├── worker/              the server: src/, migrations/ (the database), test/, wrangler.toml
├── tools/               build_pages.py (templates → public), build_artifact.py (a preview copy)
└── README.md
```

| File | What it is for |
|---|---|
| `public/assets/site.css` | the look of every page (colours, header, menu, forms) |
| `public/assets/site.js` | the menu with every page, the bottom bar on phones, the reading line, back to top, counters, and `AsideAPI` (the calls to the server, with Turnstile) |
| `public/assets/data.js` | the numbers the pages show **when there is no server** (a page opened from disk, a preview): the beta round and the roadmap. With the server the pages read them from it and you change them in the admin. |
| `templates/<name>.html` | one generated page: a few lines of front matter (title, description), its own CSS, its body and its script. `python tools/build_pages.py` writes `public/<name>.html`. |

The rule: **edit `templates/` and run `python tools/build_pages.py`**; edit the four hand-written pages and `assets/` directly. The checks on GitHub fail if a template changed but the page was not rebuilt.

Without the server every form still works by e-mail (it opens the visitor's mail app with the answers filled in); with it the forms send straight from the page.

## What the server does

- **The admin** (`/admin`): behind Cloudflare Access (your e-mail and a code), optionally also an emergency token; works on a phone. Sections: Overview (what waits for you), Applications, People, Bug reports, Feedback, E-mails, Messages, Roadmap, Showcase, Releases, Data, Log, Settings, and a search over everything (`/` on a keyboard).
- **Beta applications** (`beta.html`): the applicant gets an e-mail with a confirmation link (`beta.html#confirm=…`, a button confirms it, so mail scanners cannot); only confirmed applications reach the admin and you, unconfirmed ones are deleted after 48 hours. Limits: one open application per e-mail and program, 2 a day per address, and a daily total for everyone (Settings, 30 by default). In the admin you accept, put on the waiting list or decline, with an e-mail; same-address applications are marked and can be deleted together.
- **People**: everyone accepted, with their bugs, last visit of their page, feedback state and last e-mail. An accepted person can be written to, given a new personal link (the old one stops working) or removed from the program (with an e-mail or without; the link stops working and the place is free again), and later taken back. Every person has a timeline: e-mails (with their text), decisions, bug reports, feedback.
- **Sponsored licences**: accepting starts the 30-day trial; the server mails the feedback form after 14 and 28 days (with one reminder 4 days later) and shows in the admin when both answers are in ("Ready for full licence"). The licence itself is still issued, and revoked, in the licence admin.
- **Bug reports** (`report.html#<code>`): accepted testers get a personal link; reports with a screenshot. You set the state, write an answer the tester sees on their page (optionally also by e-mail) and a private note; screenshots and reports can be deleted.
- **E-mails**: write to one person, all testers, all licences, everyone in the program or the waiting list, from saved templates (`{nick}` and `{link}` are filled in). Every e-mail the server sends is kept in a history, failed ones are flagged.
- **Data**: the size of the database, clean-up (screenshots of closed reports, declined or unconfirmed applications, old e-mails, archived messages, the log, votes), bulk delete in every list, CSV exports and a full JSON backup. Screenshots of closed reports are deleted by the daily job after 30 days (Settings).
- **Personal pages**: a wrong link is counted per address (20 an hour, then a pause), and all personal pages can be switched off at once in Settings.
- **Roadmap votes**: live counts, one vote per browser; cards are edited in the admin.
- **Showcase**: translations sent through a form, shown after you approve them.
- **Contact form**: messages in the admin; reply from the admin (sent from aside@filiposakkq.dev) or from your mail app.
- **Releases**: entered in the admin; the Releases page shows them, downloads go through `/api/download/<id>` and are counted.
- **Log**: what was done in the admin, by whom, and what the daily job did.

Every e-mail goes from `Filip from Aside <aside@filiposakkq.dev>` with replies to the same address, so answers of people reach you through the existing forwarding. Until the domain has a sending reputation, these e-mails often land in spam; the pages tell people to look there.

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
   - application domain `aside.filiposakkq.dev`, two rows: path `admin` (no slash at the end: `admin/` would leave `/admin` open to the token page only) and path `admin/*`
   - policy: Allow, Include, Emails, your address
   - session duration as you like (for example 1 month)
9. From the application's overview copy the **Application Audience (AUD) Tag**, and your team domain (`<team>.cloudflareaccess.com`, Zero Trust, Settings, Custom Pages). Put them into `ACCESS_AUD` and `ACCESS_TEAM_DOMAIN` in `worker/wrangler.toml` and merge.
10. Open `https://aside.filiposakkq.dev/admin`: Cloudflare asks for your e-mail, sends a code, and the admin opens. The server checks the Access signature itself, so if Access were ever switched off by mistake the admin stays locked.

The overview of the admin warns when e-mail or Turnstile is not set up.

## After a change

A merge to `main` that changes `web/` deploys by itself (the folder `public/` and the server). A pull request runs the checks only. To deploy again by hand: Actions, Deploy website, Run workflow (works in the GitHub app).

The checks also fail when a template changed but `python tools/build_pages.py` was not run; run it and commit the pages it wrote. New tables (a new file in `worker/migrations/`) are created by the deploy.

The daily job (feedback forms and reminders, removing unconfirmed applications and old screenshots, clean-up) runs by itself at 08:17 UTC.

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

The server stores what people type into the forms, a salted hash of their address for the flood limits (removed after two days) and to mark applications from the same address, a salted hash of a random browser id for the votes (the salt is `HASH_SALT`, known only to the server), a copy of every e-mail it sent, and a log of admin actions. No cookies, no analytics. An application can be deleted in the admin (with its feedback, bug reports and e-mail history) when its author asks; the Data section cleans up the rest.

The texts and numbers of the pages come from docs/RUST-PORT.md, docs/GAME-TESTS.md, docs/FEATURES.md and docs/PRIVACY.md; update them when those change.
