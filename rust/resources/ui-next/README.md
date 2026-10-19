# ui-next: the new interface (under development)

This folder is the **new** interface of Aside. The **classic** interface (the one that is in use) is `../ui-classic`; it is not touched by the work here, so the two can be run side by side and compared.

- Served by the server at **`/next/`** (the classic one at `/`); both talk to the same Core through `/api`, so the same project opens in both.
- Today it holds only a placeholder page. The new interface (a build of the sources, which will live in `ui-next-src/` when the work starts) is put here as plain files: `index.html` and what it needs (`assets/…`). `__TOKEN__` in `index.html` is replaced by the token of the run, like in the classic one. Links inside it must be absolute (`/next/assets/x.js`).
- When the new interface does everything the classic one does (the checklist is `docs/project/UI-PARITY.md`), it replaces the classic one: this folder becomes `ui`, and the classic one stays in git under the tag `ui-classic-1.0.0`.

The foundation of the look is here: `tokens.css` (colours, type, spacing), `components.css` (the components) and `styleguide.html` (open `/next/styleguide.html` on the running server). Rules and reasons: `docs/project/DESIGN-SYSTEM.md`.

See `docs/project/UI-NEXT.md`.
