# Design system of the new interface

The foundation the new interface ([UI-NEXT.md](UI-NEXT.md)) is built on: the colours, the type, the spacing and the components, and the rules for using them. It is code and a document that say the same thing:

- **`rust/resources/ui-next/tokens.css`** – every colour, size and duration as a named variable (the only place with values);
- **`rust/resources/ui-next/components.css`** – the components, which read only those names;
- **`rust/resources/ui-next/styleguide.html`** – the living page that shows all of it (open `/next/styleguide.html` on the running server). Its contrast table is calculated from the real values, so a colour that stops being readable shows as failed at once.

It is plain CSS with variables: it does not depend on Vue or any framework, and the same files serve a hand-written interface or a Vue one.

## Principles

1. **Dark red, one accent.** The window is a dark, warm near-black; red is the only colour that is not grey. Red means "this app": the filled button, progress, the active item. There is no green: *translated* is shown in the accent, *to do* in amber, *checked* in light grey.
2. **Calm and dense.** Translators read for hours. Text is 13–14 px on a quiet background; one main button per view; nothing moves unless it explains something.
3. **Same thing, same look.** A state, an action or a number looks the same everywhere (a progress bar is always the same bar). That is what the tokens and the components are for.
4. **Things that belong together are in one place** (the navigation is in groups; Tools holds the bulk edits; Release holds building and publishing).
5. **Nothing is lost.** The new interface does what the classic one does ([UI-PARITY.md](UI-PARITY.md)).
6. **Pro is shown quietly.** A Pro feature is in its place with the small tag *Pro* (a lock in Free); a click explains, it never interrupts.

## Colour

Named by purpose. A component never uses a hex value.

| Group | Tokens | Use |
|---|---|---|
| Surfaces (back to front) | `--bg-app` `--bg-rail` `--bg-bar` `--bg-card` `--bg-hover` `--bg-input` `--bg-code` | the window, the navigation, the bars, a card or a field, a hovered or selected row |
| Lines | `--line` `--line-soft` `--line-accent` | a border, a divider between rows, a border of something to notice |
| Text | `--text` `--text-2` `--text-muted` `--text-faint` `--text-disabled` | the main text, secondary, supporting, hints and column headers, disabled |
| Accent | `--accent` `--accent-hover` `--accent-hi` `--accent-soft` `--accent-text` `--on-accent` | the filled button, progress and marks, a chip or the active item, text in the accent colour, text on the button |
| States | `--todo` `--checked` `--danger` (+ `-bg`) `--track` | to do, checked, a blocking error, the empty part of a bar |

**Contrast** (WCAG, measured by the styleguide): text is at least 4.5 : 1 on its surface; a control or a mark at least 3 : 1. The one text colour that is easy to get wrong is `--text-faint` (hints, headers of columns): it is `#8f787b`, 4.55 : 1 on a card. `--text-disabled` is 3.2 : 1: for disabled controls and decoration only, **never for the only copy of a message**. An earlier sketch used a dimmer grey for hints; it did not pass (3.6 : 1) and was replaced.

## Type

The system font (`system-ui`, Segoe UI on Windows): nothing is shipped or loaded, so the application starts offline and fast. Two weights: **400 and 500** (never bold). Monospace for code and references (`script.rpy:48`).

| Token | Size | Use |
|---|---|---|
| `--fs-hero` | 22 | the name of the product, the one big number |
| `--fs-title` | 19 | the title of a screen |
| `--fs-heading` | 16 | the heading of a card or a dialog |
| `--fs-text` | 14 | reading text, names |
| `--fs-body` | 13 | the dense interface: tables, lists, buttons |
| `--fs-small` | 12 | metadata, chips |
| `--fs-caption` | 11 | headers of columns, badges |

Sentence case everywhere (Czech: only the first word and proper names are capitalised). **Every label begins with a capital letter, including states, tags and chips** (*K překladu*, *Přeloženo*, *Zkontrolováno*, *Nové rozhraní*), so a row of them reads evenly; `.pill` and `.tag-pro` also enforce it in CSS. No terminal punctuation on labels and headings. Wording follows the rules of the content below.

## Space and shape

- **Spacing:** steps of 4 px (`--s-1` … `--s-10`: 4, 8, 12, 16, 20, 24, 32, 40). Inside a card 16; between cards 12–14; the screen's margin 20–24.
- **Corners:** 6 (tags in a table), 8 (buttons, fields, rows), 12 (cards), 14 (the main card, a dialog), pill for chips and states.
- **Controls:** 32 px high (28 small), a row of a table 36 px.
- **Elevation:** flat. Only floating things (menu, dialog, toast) have a border in the accent line and a shadow, and **at most two floating layers** are on screen at once.

## Components and their states

Every component has: default, hover, focus (a visible ring), disabled; and where it makes sense selected, invalid and loading.

| Component | Class | Rules |
|---|---|---|
| Button | `.btn` + `.primary` `.ghost` `.danger` `.sm` | one `.primary` per view; a destructive action is `.danger` and asks first |
| Filter | `.chip` `.chip.on` | a control: toggles a filter; the active one is in the accent |
| State | `.pill` + `.todo` `.ok` `.checked` `.danger` `.none` | only tells; the same word and colour for the same state everywhere |
| Badge | `.badge` | a count that wants attention (errors, news); hidden at zero |
| Pro tag | `.tag-pro` | on a Pro feature; a lock in Free |
| Card | `.card` `.hero` `.active` | the basic surface; a title with an optional link on the right |
| Field | `.field` `textarea.field` `.label` `.hint` `.invalid` | the label above, the hint below; an error in text, not only in colour |
| Progress | `.bar` `.thin` `.two` | the same bar for translated and checked (`.two`) |
| Notice | `.notice` `.warn` `.danger` | one sentence and at most one action; no pop-ups |
| Navigation | `.nav` `.nav-item` (project), `.rail` `.rail-item` (Home) | the active item has the soft accent fill |
| Tabs | `.tabs` `.tab` | for sections of one detail |
| Table row | `.row` `.head` `.sel` | 36 px; the selected row has an accent stripe on the left |
| Menu | `.menu` `.menu-item` | opens from a button; one level |
| Dialog | `.veil` `.dialog` | one at a time; the main action on the right; Esc closes |
| Toast | `.toast` | a short confirmation, never for an error that needs a decision |
| Skeleton | `.skeleton` | a placeholder while loading, instead of a spinner |

## Motion

Short and purposeful: 120 ms for hover, 200 ms for state changes, 500 ms for a bar that fills and for the start animation. The start animation (the mark, the name, a thin bar) is about 3 seconds, **only when the library is loading**, and does not hold anyone up. All motion is off when the system asks for less (`prefers-reduced-motion`): the tokens become 0 ms.

## Focus and keyboard

Every control can be reached and used with the keyboard; the focus ring (`--focus`) is visible on every surface and is never removed. The keyboard shortcuts of the classic interface stay as they are ([UI-PARITY.md](UI-PARITY.md), area *Klávesové zkratky*).

## Icons

One set: **Tabler Icons (outline, MIT licence)**, shipped with the application as files (no loading from the internet), sized 16–20 px, in the colour of the text around them. Not drawn by hand. (The styleguide shows no icons yet: the files are added when the first real screen is built.)

## Content (the words)

Plain, short, friendly. Verb first on buttons (*Přidat hru*, *Odeslat změny*); a state is a short noun or adjective (*k překladu*, *zkontrolováno*). No *prosím*, no exclamation marks, no *úspěšně* (the result is its own confirmation). An error says what happened and what to do, in one sentence, with no code. Empty states invite (*Přidej první hru*), they do not apologise. The English text is the source; the Czech text is the one that is read first.

## Responsive

The window is resizable down to about 900 px wide. Below that the right-hand detail goes under the list; the navigation shrinks to icons (like the rail of Home). Tables scroll sideways inside their card, never the page.

## What changes this document

A new colour, size or component is added to `tokens.css` or `components.css` and shown in the styleguide **in the same change**; this page gets a line. The contrast table must stay green.
