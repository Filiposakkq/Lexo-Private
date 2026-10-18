# Aside website

A static one-page site for Aside (no build step): `index.html` is the landing page, `docs.html` the documentation, `site.css` their shared styles; `img/` the screenshots of the app (a made-up demo project), `og.png` the social preview, `icon-*.png` and `favicon.svg` the icons (rendered from `free/assets/brand`).

Open `index.html` in a browser, or serve the folder with any static server.
`python web/tools/build_artifact.py OUT_DIR` makes a preview copy (index with styles and images inlined, docs next to it).

The numbers on the page come from docs/RUST-PORT.md (Progress), docs/GAME-TESTS.md, docs/FEATURES.md and docs/PRIVACY.md; update them when those change.
The links (Ko-fi, filiposakkq.dev, the Being a DIK translation) are the ones of the earlier draft on `feat/web`.
