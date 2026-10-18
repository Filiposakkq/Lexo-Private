# Aside website

A static one-page site for Aside (no build step): `index.html` is the landing page, `releases.html` the releases with the changelog and statistics, `docs.html` the documentation, `site.css` their shared styles; `img/` the screenshots of the app (a made-up demo project), `og.png` the social preview, `icon-*.png` and `favicon.svg` the icons (rendered from `free/assets/brand`).

Open `index.html` in a browser, or serve the folder with any static server.
`python web/tools/build_artifact.py OUT_DIR` makes a preview copy (index with styles and images inlined, docs next to it).

The numbers on the page come from docs/RUST-PORT.md (Progress), docs/GAME-TESTS.md, docs/FEATURES.md and docs/PRIVACY.md; update them when those change.
The links (Ko-fi, filiposakkq.dev, the Being a DIK translation) are the ones of the earlier draft on `feat/web`.

Sub-pages `compare.html`, `blog.html` and `developers.html` are assembled from `pages/` (shared header and footer in `pages/_head.html` and `_foot.html`) by `python web/tools/build_pages.py`; edit the files in `pages/`, not the generated ones. `site.js` holds the mobile menu and the counters of the sub-pages; `badge/` the Translation ready badges.
`beta.html` has two applications (testing, free licence) as step-by-step forms with required fields; the last step opens an e-mail to aside@filiposakkq.dev with the answers (and offers to copy them). `roadmap.html` keeps the visitor's votes in the browser and sends them by e-mail; `showcase.html` lists translations made with Aside.
