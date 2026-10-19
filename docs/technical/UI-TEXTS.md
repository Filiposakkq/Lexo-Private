# Interface texts (translation of the app itself)

The app is **English first**. English needs no table: **the English text is the key** (`t('Overview')`). Another language is a language pack with a `ui.json`
(`rust/resources/packs/cs/ui.json` is the first one); the server hands it out (`/api/ui-lang?code=cs`, list: `/api/ui-langs`).

## In the code (`rust/resources/ui-classic/js/05-i18n.js`)
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

## Messages from Python (`reference/python/core/lexo/messages.py`)
Every sentence the Python side shows to the user is written once, in English, as a template: `msg("Cannot save {name}: {reason}", name=n, reason=r)`;
counted messages `msgn(n, "{n} line", "{n} lines")`; several messages one after another `join([...], ", ", ".")`. `str(message)` is the English text
(logs, the command line, tests). On its way to the browser the server turns messages into `{"$m": template, "p": values}` (`wire`), and `api.js` translates
them with the same `ui.json` as the UI texts (`fromWire`), so the rest of the UI sees ordinary texts. Rules: the template is a plain string literal; values
(also tags such as "{/b}" and paths) go in placeholders; an exception carries its message as `AppError(msg(...))` (use `text_of(e)` to re-raise without losing it).
Tests in Python use `render(...)` to read what the server sent.

**What stays Czech in the code on purpose** (data stored in files or identifiers, listed in `tests/test_python_messages.py`): the Czech letters in `rpy.py` that the line classifier looks for
(language pack material, step 3). The kinds of pending lines are codes (`empty`, `english`, `same`, `symbols`) that the UI words; the CSV status words and the text-history sources are already English / messages (see DATA-LAYOUT.md).

## State
Everything the user sees is translatable: all modules of the browser UI, `index.html`, and the Python-side messages. The English UI shows no Czech except the language names.
Still open: `selftest.js` checks Czech texts on purpose (it runs with `?lang=cs`); the "About" text of the settings still describes the first game; Czech typographic snippets
(quotation marks) in the translation editor belong in the language pack; the command line prints English only; an input keyword ("ano" to mark a whole file as proofread) is Czech.

Tools used for the conversion live outside the repository (a scanner, converters that wrap literals / f-strings and write `ui.json`); the tests keep the result honest:
`test_ui_i18n.py` (keys, translations, placeholders, no Czech literals in the UI; also the templates of Python messages), `test_ui_i18n_load_time.py` (nothing is translated
while a module loads), `test_python_messages.py` (no Czech sentences in Python), `test_messages.py`.
