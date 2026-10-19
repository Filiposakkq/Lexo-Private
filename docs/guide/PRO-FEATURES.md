# Pro features: what they do and how to use them

For the user's view of Pro (what it costs, the licence, the computers) see the page of the website; for how Pro is built and kept apart see [PRO-DESIGN.md](../product/PRO-DESIGN.md); for the status
of every feature see [FEATURES.md](../product/FEATURES.md). This document says what each feature does, where it is in the application, what it needs, what it sends over the network and how to undo it.

**In general**
- Without a licence the "taste" features work on a small batch (the demo limits below); the premium features are locked and say so.
- Every feature that changes the files shows a **preview** first; nothing is written until you tick lines and confirm. A write is **one undoable step** with a backup of the files
  (`Undo the last file change` in the menu *More* of the line editor; marks and approvals have the normal undo, Ctrl+Z).
- Lines that are **approved** are never overwritten by a machine, a fill or a fix.

| Feature (id) | Where | Demo (no licence) | Needs | Sends text over the network |
|---|---|---|---|---|
| Batch machine translation (`mt.batch`) | Lines › Tools › *Translate with a machine…* | 20 lines per run | a service and a key in Settings, the consent | **yes**, to the service you chose |
| Safe bulk fixes (`fixes.safe`) | Lines › Tools › *Fix…*, the Check tab | 20 items per run | – | no |
| Regex find and replace (`replace.regex`) | Lines › Tools › *Find and replace…*, mode *Regular expression* | locked | – | no |
| Group suggestions (`bulk.categorize`) | Files › groups | 5 files | – | no |
| Publishing progress (`publish.web`) | Publishing tab | locked | a git repository you own | git push to **your** repository |
| Carry a translation over to a new version (`carry.over`) | Lines › Tools › *Carry over from the previous version…* | 10 lines written (the counts are complete) | the folder of the old version | no |
| Fill from similar translations (`memory.fuzzy`) | Lines › Tools › *Fill from similar translations…* | locked | – | no |
| Profiles of characters (`mt.profiles`) | Glossary › *Profile…* per speaker | locked | a language model for the batch translation | with the batch translation only |
| Style check (`style.check`) | Lines › Tools › *Check the style…* | locked | an OpenAI-compatible service, the consent | **yes**, to that service |
| Spelling with a dictionary (`spell.hunspell`) | Lines › Tools › *Check the spelling…* | locked | a Hunspell dictionary added by you | no |
| Release builder (`release.build`) | Lines › Tools › *Build a release…* | locked | – | no (nothing is uploaded) |

## Batch machine translation
Translates the untranslated lines of the selection, the scene around the open line (±15), a file or the whole project, in small batches with progress and a Stop button. The suggestions
are shown in a table you can edit; only the ticked ones are written. Tags and variables are protected and restored, and the validator checks every suggestion.

**What the service gets:** the source text of the line; for a **language model** (OpenAI-compatible) also the two lines before it (original, "do not translate"), the glossary terms that occur in
the line and, with the profiles, a description of the speaker. DeepL and LibreTranslate get only the text of the line (no context, no glossary): they are translation services, not models.
Settings › Machine translation: the service, the key (it stays on your computer), the consent, the target language (empty means the language of the project).

## Safe bulk fixes
Fixes that cannot break a tag or a variable, each with a preview of every change: the **speaker** and attributes copied from the original, **double spaces**, **typos** (suggestions from the
dictionary of your own translation; the uncertain ones start unticked). Undo: the backup of the files (`Undo the last file change`).

## Regex find and replace
Plain and whole-word search and replace are free; the **regular expression** mode (with `\1` groups) is Pro. Also with a preview, per line ticks and one undo step.

## Group suggestions
Proposes which files belong to which group (episode, chapter) from their names and folders; applying a chosen assignment is free.

## Publishing progress
Writes the progress file (and a page for a website) into a git repository you own and pushes it, after you confirmed the repository, the branch and what changes. Nothing is cloned or pushed
without the confirmation; a failed push restores the local commit and the file.

## Carry a translation over to a new version of a game
The open project is the **new** version, you choose the folder of the **old, translated** one. Every untranslated line of the new version is looked up in the old one **by its original text**
(the ids of the scenes change between versions):
- **same** – the same words, the same speaker and the same variables: the translation is carried over as it is;
- **changed N %** – the original differs by a few words (from 80 %, the same speaker): carried over, shown with the difference; if the variables or tags differ it says so;
- no match: stays untranslated.

Lines that were carried over from a **changed** original are marked **to fix** (a tick box, on by default) so you find them in the filter *To fix*. Measured on edits of real translations:
about 98–100 % of the found lines get the right translation, wrong matches 0–1 %; edits a little or somewhat bigger are found in 77–96 %.

## Fill from similar translations (fuzzy translation memory)
The same dialog and matcher, with the project **itself** as the source: every untranslated line gets the translation of a similar original of the same speaker that is already translated (from 85 %).
The one-line suggestion of similar originals in the line editor (the Memory panel) stays free. Filled lines are marked **to fix**.

## Profiles of characters
For a speaker: the **gender**, **how the character speaks** (free text) and **how they address the others** (informal *tykání* or formal *vykání*). Saved as a hidden last line `@profile …` of the note
of the glossary entry, so it needs no new file and travels with the glossary (also between translators); the glossary page shows and saves only the free text. The batch machine translation
sends the profile with every line of that speaker, so that a language model keeps the feminine and masculine forms and the form of address right. DeepL and LibreTranslate cannot use it.

## Style check
Sends each translated line, with its original, the two lines before it and the profile of the speaker, to the language model of the machine translation (OpenAI-compatible service only), and shows
what is wrong: the **form of address**, the **gender forms**, the **tone**, the **meaning**, with a corrected text. Lines without a remark are only counted. The corrections you tick are written like a
machine translation (one undo step). The service charges by the text sent (the dialog shows how much will be sent). The quality of the remarks depends on the model; read them before ticking.

## Spelling with a dictionary
Checks the translated lines against a **Hunspell dictionary** (a `.dic` file with its `.aff` file) of the language of the project. The free check from the project's own vocabulary stays as it is; this one knows real words, so it also finds words that are wrong everywhere in the project.
- **Add a dictionary:** *Lines › Tools › Check the spelling…*, then choose the folder or the `.dic` file (the `.aff` file lies next to it). Both files are copied to the settings of the application (one dictionary per language of a project; not shared with the project). A broken dictionary is reported when it is added. The application does not include or download any dictionary.
- **Check:** the file in the filter or all files. The list shows every unknown word with how often and where it occurs and with suggestions (words one edit away, and the word with its diacritics put back). Words of the original, words of the glossary, short words in capitals, tags and variables are never reported.
- **Correct:** *Replace…* plans the lines first (the word whole, never inside `[variables]` or `{tags}`); one click writes them as one undoable step; approved lines are never overwritten. *Ignore* remembers a word in the project (`.aside/pro/spelling.json`, shared).
- **What is read:** the flag formats (ASCII, UTF-8, long, num), prefixes and suffixes with their conditions and the cross product, one level of continuation flags, `NEEDAFFIX`, `ONLYINCOMPOUND`, the encodings UTF-8, ISO-8859-1 and ISO-8859-2. Compound words, `REP` and `ICONV` tables and `KEEPCASE` are not used (a language that is built on them may show some words as unknown).

## Release builder
Makes the translation into **a zip the players unpack into their game folder**; the result is a file and you put it where you like (nothing is uploaded).
- **Which files:** a preset, **files translated at least partly** (the default), **only files translated completely**, or **all files**; on top of it you can tick or untick any single file in the list (the dialog shows how far each file is translated). Skipped files (Settings) never go in.
- **Inside the zip:** the `.rpy` files of the translation as `game/tl/<language>/…` (the `.aside` folder and the backups never go in), **the files you attach** (a font with the diacritics of your language, a changed gui file...: each with the place you give inside the game folder), `README.txt` (how to install, credits and a note of yours; in Czech for a Czech translation), `CHANGELOG.txt`, and, if you leave it on, `game/zz_<language>_default_language.rpy` (`define config.default_language`) so the game starts in your language.
- **Next to the zip:** `<name>.zip.sha256` and `<name>-release-notes.md` (Markdown for a page: GitHub, F95, Patreon).
- **Your own numbering:** `1.0`, `1.2.3`, `1.2-beta1`; the next number is suggested; a number is released once.
- **Checks before the build:** the number is valid and new; errors of the check of the translation (they can be built anyway after you tick it); untranslated lines (the players will see English there); lines flagged "to fix" or changed after proofreading; the game version changed since the last release (read from `config.version`); the attached files exist, sit inside the game folder and weigh together at most 300 MB.
- **Changelog:** compared with the previous release: files added, changed and removed, and (when the previous release has its line digests, kept in the local data) how many lines are new, changed or removed; your own notes first; the progress.
- **Recorded in** `.aside/release.json` (the settings and the list of the releases, shared with the project); the zip goes to the folder you choose (default: the local data of the project, `releases/`).
- Not there yet: `.rpa`, compiled `.rpyc`, a zip with only the files changed since the last release, a check that the font has the characters of your language.

## Limits, in short
| Feature | Demo limit |
|---|---|
| Batch machine translation, safe fixes | 20 per run |
| Carry over | 10 lines written per run (the counts show all) |
| Group suggestions | 5 files |
| Regex replace, publishing, profiles, release builder, fill from similar translations, style check, spelling with a dictionary | locked |

A licence removes the limits and unlocks the locked features (see [PRO-DESIGN.md](../product/PRO-DESIGN.md) for the licence, the computers and the checks).
