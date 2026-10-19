# Two interfaces side by side: classic and next

The interface is being redesigned (sketches, the list of what must stay: [UI-PARITY.md](UI-PARITY.md)). While that happens **the old interface stays untouched and usable**, and the new one grows beside it, so the two can be compared at any time on the same project.

## What is what

| Name | Where | URL (server) | State |
|---|---|---|---|
| **classic** | `rust/resources/ui-classic/` | `/` | the interface in use; only bug fixes and what parity needs |
| **next** | sources `ui-next-src/` (Vue 3 + Vite) → built into `rust/resources/ui-next/` | `/next/` | the new interface, **under development**: Home is built (the library of games); the screens of a game are still the classic ones |
| archive | git tag `ui-classic-1.0.0` | – | the classic interface exactly as of version 1.0.0 |

The names are the folder names, so nothing is confusable: `ui-classic` is the old one, `ui-next` is the new one. After the switch (below) `ui-next` becomes `ui` and `ui-classic` is gone from the tree (it stays in the tag).

## How to run each

- **Launcher** (`tools/launcher/Spustit-test.cmd`): the box *Nove rozhrani (/next/) misto klasickeho* opens `/next/` instead of `/` (in the browser mode; the window of the application, Tauri, shows the classic one).
- **By hand:** start `lexo-server` and open `/` (classic) or `/next/` (new). Both talk to the same Core through `/api` and use the same token, so the same project works in both.
- **Folders:** `LEXO_UI_DIR` replaces the classic folder, `LEXO_UI_NEXT_DIR` the new one (`lexo-server --ui <folder>` too, for the classic one). `/next/` exists only when the folder has an `index.html`; otherwise it is "not found".

## The new interface: sources, build, tests

- **Sources:** `ui-next-src/` (Vue 3, plain JavaScript, Vite). `src/App.vue` is the shell, `src/views/` the screens (Home so far), `src/components/` the parts, `src/i18n.js` a copy of the text module of the classic interface (the same rule: the English text is the key; the same Czech pack `packs/cs/ui.json`), `src/api.js` talks to `/api` with the token of the run. `public/` holds `tokens.css`, `components.css` and the styleguide, which are copied as they are ([DESIGN-SYSTEM.md](DESIGN-SYSTEM.md)).
- **Build:** `cd ui-next-src && npm install && npm run build` (Node is needed only for this). It writes `rust/resources/ui-next/` (git ignores it, only its README stays); the launcher has a box *Nejdriv sestavit nove rozhrani*. The built files are plain static files, so the application never needs Node or Vue at run time.
- **Develop:** `npm run dev` serves the sources at `http://localhost:5173/next/` and forwards `/api` to a running server (`VITE_API=http://127.0.0.1:<port>`).
- **Tests:** `npm test` (Vitest + Vue Test Utils); the workflow *New interface* runs the tests and the build. The Czech texts of the new interface are checked by the same test as the classic one (`test_ui_i18n`: every text has a Czech text, nothing is stale).
- **Library of games:** Home reads `library/list` (`library/add`, `library/remove`, `library/pin`): a local file `library.json` in the settings of the application, never in a project; a project that is opened is put on Home by itself. Opening a game calls `project/set` and continues in the classic interface (until the screens of a game exist in the new one).

## Rules while both exist

1. **The classic interface is not restyled or rebuilt.** Only bug fixes, and changes that a Pro feature or the contract needs (a new feature appears in both until the switch).
2. **The new interface does through `/api` exactly what the classic one does.** Every action of the classic one is a line of [UI-PARITY.md](UI-PARITY.md), with the place where the new interface has it. Nothing is removed from the classic one before its line is filled.
3. **Links inside the new interface are absolute** (`/next/assets/x.js`), and `__TOKEN__` in its `index.html` is replaced by the token of the run, like in the classic one.
4. **The installer ships the classic interface only** (`tauri.conf.json` bundles `ui-classic` as `ui`). The new interface is added to the installer when it is ready to be tried by users, as a choice, and replaces the classic one at the switch.
5. The tests (the UI self-test, the contract test of the routes, the texts) run against the classic interface until the new one has its own self-test; the new interface gets the same checks before the switch.

## The switch (when the new interface is done)

1. Every line of the parity list is filled and ticked; the new interface passes the same self-test as the classic one.
2. `rust/resources/ui-next` is renamed to `rust/resources/ui` (and its build output is what is shipped); `ui-classic` is deleted from the tree. The tag `ui-classic-1.0.0` keeps it.
3. The paths in `tauri.conf.json`, the workflows, `lexo-server`, the Python tests and the launcher are changed back to `ui` (they were changed from `ui` to `ui-classic` when this layout was made, and the new folder was added).

## About the framework

The new interface may be written in a framework (Vue was discussed). It does not matter for this layout: whatever is built is copied into `ui-next/` as plain files (`index.html`, `assets/…`), and the sources go to their own folder (planned: `ui-next-src/`), so the application and the installer never depend on a build tool at run time.
