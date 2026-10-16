# Interface texts (translation of the app itself)

The app is **English first**. English needs no table: **the English text is the key** (`t('Overview')`). Another language is a language pack with a `ui.json`
(`free/lexo/packs/cs/ui.json` is the first one); the server hands it out (`/api/ui-lang?code=cs`, list: `/api/ui-langs`).

## In the code (`free/lexo/ui/js/05-i18n.js`)
| Call | Use |
|---|---|
| `t('Text {name}', { name })` | a text with placeholders |
| `tn(n, '{n} file', '{n} files')` | a counted text; the pack gives one text per plural form (key `"{n} file|{n} files"`, a list) |
| `tx('Folder {path} ...', { path: h('b', null, p) })` | like `t`, but the values may be DOM nodes; returns an array for `h()` children |
| `labels({ done: 'Done', ... })` | a fixed table translated when it is READ (the language is chosen after the modules load); use it for module-level constants |
| `data-i18n="Text"`, `data-i18n-title`, `data-i18n-aria` in `index.html` | static texts |
| `get label() { return t(...) }` | a text inside an object built at load time |

Rules: the key is a plain string literal, never a template literal or a concatenation; a text with a variable part uses a `{placeholder}`; one sentence is one key
(never glue translated pieces together, word order differs between languages). Numbers and dates follow `locale()`.

## Language choice
`?lang=cs` in the address (not remembered) > the saved preference `ui.lang` > the browser's language > English. A text without a translation shows in English.

## Pack side
`ui.json`: `{"strings": {"English text": "translation" | ["form 1", "form 2", ...]}}`. `rules.json` names the plural rule (`"plural": "cs"`; the rules live in
`lang.PLURAL_RULES` and in `05-i18n.js`, a test keeps them equal). Placeholders of a translation must be those of the English text.

## Checks (`tests/test_ui_i18n.py`)
every `t()` text has a Czech translation, the pack has no stale texts, placeholders match, and the modules listed in `CONVERTED` contain no Czech string literal.
**Converting a module** = move its Czech literals to `t()` and add them to `ui.json`, then add the module to `CONVERTED`.

## State
**Every module of the browser UI is converted** (the test `CONVERTED` list is all files of `ui/js`). What is still Czech:
- `selftest.js` (it checks Czech texts on purpose and runs with `?lang=cs`);
- the **messages of the Python side** (errors, validator findings, labels such as the episode states, history labels, the text of `progress.json` hints); they will become message keys. In an English UI these still appear in Czech;
- places where the UI compares server text (for example the publishing warning regex looks for the Czech word "ostro") and an input keyword ("ano" is sent to the server to mark a whole file as proofread);
- the "About" text of the settings (it still describes the first game) and a few product constants (links to the BaDIK site);
- Czech typographic snippets (quotation marks) in the translation editor belong in the language pack later.

Tools used for the conversion live outside the repository (a scanner, a converter that wraps literals and writes `ui.json`); the tests keep the result honest: `test_ui_i18n.py`
(keys, translations, placeholders, no Czech literals), `test_ui_i18n_load_time.py` (nothing is translated while a module loads).
