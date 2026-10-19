# Two interfaces side by side: classic and next

The interface is being redesigned (sketches, the list of what must stay: [UI-PARITY.md](UI-PARITY.md)). While that happens **the old interface stays untouched and usable**, and the new one grows beside it, so the two can be compared at any time on the same project.

## What is what

| Name | Where | URL (server) | State |
|---|---|---|---|
| **classic** | `rust/resources/ui-classic/` | `/` | the interface in use; only bug fixes and what parity needs |
| **next** | `rust/resources/ui-next/` | `/next/` | the new interface, **under development** (today a placeholder page) |
| archive | git tag `ui-classic-1.0.0` | – | the classic interface exactly as of version 1.0.0 |

The names are the folder names, so nothing is confusable: `ui-classic` is the old one, `ui-next` is the new one. After the switch (below) `ui-next` becomes `ui` and `ui-classic` is gone from the tree (it stays in the tag).

## How to run each

- **Launcher** (`tools/launcher/Spustit-test.cmd`): the box *Nove rozhrani (/next/) misto klasickeho* opens `/next/` instead of `/` (in the browser mode; the window of the application, Tauri, shows the classic one).
- **By hand:** start `lexo-server` and open `/` (classic) or `/next/` (new). Both talk to the same Core through `/api` and use the same token, so the same project works in both.
- **Folders:** `LEXO_UI_DIR` replaces the classic folder, `LEXO_UI_NEXT_DIR` the new one (`lexo-server --ui <folder>` too, for the classic one). `/next/` exists only when the folder has an `index.html`; otherwise it is "not found".

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
