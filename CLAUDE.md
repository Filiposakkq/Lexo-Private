# Aside (code name Lexo)

Map of the repository: `README.md`; index of the documents: `docs/README.md`.
Read `docs/project/HANDOFF.md` first: what the project is, decisions already made, safety rules, the step loop with checkpoints, commands, current state.
Then `docs/project/ROADMAP.md` for the next step. Owner writes Czech; code and docs are English.
Never push, open a PR or merge without asking. Game folders are read-only.

## Response defaults (the owner's rules, apply to every reply unless overridden)
- Answer directly. No preamble, filler, affirmations or trailing summary clauses.
- Plain prose or tight lists. No decorative headers for short answers.
- No extended thinking and no web search unless the prompt is explicitly complex or time-sensitive.
- At 15+ messages offer once to summarize the key context for a fresh chat.
- If the owner asks for a correction, note once that editing the last message saves tokens.

## Where the work stands (new interface, `ui-next`)
- Built in Vue (`ui-next-src/`, output `rust/resources/ui-next`, served at `/next/`): Home, Prepare a game (with SDK download on request), Overview, Files, Lines (full detail, Tools menu with 10 dialogs), Groups, Team. Classic UI (`/`) stays untouched for comparison; the test launcher can open both side by side.
- Still classic: Check, Glossary, Publishing, Settings. Not ported yet: the scene picture in the line detail, bubbles view, second window. The list of every action is `docs/project/UI-PARITY.md`; the state is in `docs/project/UI-NEXT.md`.
- Texts: English is the key, Czech in `rust/resources/packs/cs/ui.json`; `py run_tests.py free` (in `reference/python`) fails on a missing or stale key. Reuse existing classic keys word for word.
- Verify a change: `npm test` and `npm run build` in `ui-next-src` (Node at `E:\node`), then open the real project in the browser pane on a COPY of a translation folder (never the owner's files).
- Pitfalls: write files with the Write tool (bash heredocs break on quotes); a class called `row`, `nav` or `menu` collides with `components.css`; `export` is not allowed in `<script setup>`; keep line endings of existing files.
