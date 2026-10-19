# ui-next: the new interface (under development)

This folder is the **new** interface of Aside. The **classic** interface (the one that is in use) is `../ui-classic`; it is not touched by the work here, so the two can be run side by side and compared.

- Served by the server at **`/next/`** (the classic one at `/`); both talk to the same Core through `/api`, so the same project opens in both.
- This folder is the **build** of the new interface (Vue 3 + Vite, sources in `../../../ui-next-src`): `npm run build` there writes `index.html` and `assets/` here (git ignores them; this README stays). `__TOKEN__` in `index.html` is replaced by the token of the run, like in the classic one. Links inside it are absolute (`/next/assets/x.js`).
- When the new interface does everything the classic one does (the checklist is `docs/project/UI-PARITY.md`), it replaces the classic one: this folder becomes `ui`, and the classic one stays in git under the tag `ui-classic-1.0.0`.

The foundation of the look (`tokens.css`, `components.css`, `styleguide.html`) is kept in `ui-next-src/public` and copied here by the build; open `/next/styleguide.html` on the running server. Rules and reasons: `docs/project/DESIGN-SYSTEM.md`.

See `docs/project/UI-NEXT.md`.
