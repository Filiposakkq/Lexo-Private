# Aside: everything the application can do

A reference of every function, by place in the application. For a first walk-through (installing, preparing a game, the first translation) read the documentation page of the website
(`web/public/docs.html`); for the Pro features in detail read [PRO-FEATURES.md](PRO-FEATURES.md); for what leaves your computer read [PRIVACY.md](../product/PRIVACY.md). **Pro** marks what needs a licence
(some work on a small batch without one).

## 0. The ideas behind it

| Idea | Meaning |
|---|---|
| **Translation folder** | `game/tl/<language>/`: the `.rpy` files Ren'Py reads. You open that folder (or the game folder, Aside finds the translation inside). The game itself is only read. |
| **Line** | One piece of text to translate: a **dialogue** (with a speaker) or a **choice / string** (from `translate strings`). Every line has an original and a translation. |
| **State of a line** | *to translate* (the translation is empty or the same as the original), *translated*, *approved* (you said "this is right even though it looks untranslated", for example a name), *for proofreading*, *proofread*, *to fix* (flagged with a note), *changed after proofreading* (the text changed after you marked it). |
| **Finding** | What the check says about a line: an error (the game would probably crash) or a warning (suspicious). Nothing is ever corrected by itself. |
| **Group** | A set of files with statistics of its own: an episode, chapter, part, day... Found from the file names. |
| **Glossary** | The full names of the speakers (`mc`, `de`...) and the terms of the game with their translations. |
| **Where data lives** | Your translation in `game/tl/<language>/`; small shared project files in `game/tl/<language>/.aside/` (groups, glossary, proofreading marks, one change log per author); local data (backups, snapshots, history of each line) in `%APPDATA%\Aside\projects\<id>\`. |

Every write into the files is **atomic and backed up first**. What can be undone, and how, is in section 10.

## 1. The tabs

The tabs along the top are the whole application (`Alt+1`...`Alt+7` go to them): **Overview**, **Files**, **Lines**, **Check**, **Glossary**, **Publishing**, **Settings**.
The top bar also has: the folder of the open project and *Change…*, a button to open a **second window**, **full screen**, the **Undo** button, **Refresh files**, and the help (`?`).

### 1.1 Overview
How far you are, at a glance.
- **Rings**: the outer one translation, the inner one proofreading; the number of lines left; buttons **Start translating / Continue**, **Translate quickly**, **Proofread quickly** (section 3.6).
- **Cards**: translation percentage (and how many lines were approved by hand), lines left (and files unfinished), proofreading (marked line by line, changed after proofreading), the check (errors and warnings).
- **Weekly goals** (translation and proofreading, lines per week): set in Settings, or automatic from your pace of the last weeks (+10 %). After a week of work: *pace ≈ N lines a day* and **estimated finish**.
- **Groups**: statistics per episode / chapter / part, with a selector for the **state of a group** that you set by hand when it matters; *Outside episodes* (general files, unassigned files); a button to **set up groups**.
- **Most problematic files** (problems per 100 lines, an error counts three times; a click opens the check of the file); **Files by status**; **Recent activity**; **Recent changes (can be undone)**; **Last loading of files** (and skipped files).

### 1.2 Files
A table of the translation files with their counts and states.
- Columns: lines, translated, proofread, errors / warnings, assignment to a group. Search, filter by assignment, **only untranslated**, **lines changed after proofreading**.
- **Double-click** opens the lines of a file. Right side: details of the selected file, *Open in the editor*, *Copy the path*, *Continue proofreading*.
- **Assign to a group**: for the selected files, or **Suggest assignment…** (from the file names; a preview of changes and conflicts; a manual assignment is overwritten only when you tick it). Pro: the suggestion of groups for files (5 files in the demo).
- **Manual correction of counts**: override the number of translated or proofread lines of a file by hand (a number, a percentage or "ano"), or restore the automatic value; **mark a whole file as proofread**.
- **Groups…**: what the groups are called (episode, chapter, part, day, update, act...) and which numbers exist; proposed from the names (numbers after a word, a numbered series `script`, `script2`..., one group per folder). Saved in `.aside/aside.json`.
- Another **window** of the application on the same project (the scene or the file list next to the editor).

### 1.3 Lines
The editor; see section 2.

### 1.4 Check
The findings of the whole project; see section 4.

### 1.5 Glossary
See section 5.

### 1.6 Publishing (Pro)
Writes the progress of the translation for a website. Two separate steps: **1. Generate and check the data** (a preview: translation and proofreading percentages, finish estimate, groups; the data is checked), **2. Publish** (the file in a git repository you own: you see the commands that will run, the other uncommitted changes that stay untouched, and you confirm that the data goes to GitHub and the website is updated). Nothing is sent before the confirmation; a failed push restores the local commit and the file. The sign-in is done by your own git (Credential Manager): no tokens are stored.

### 1.7 Settings
See section 8.

## 2. Lines: translating and proofreading

### 2.1 The table
Every line with its file, line number, script position, type (dialogue / choice), state, proofreading, speaker, original and translation. Columns can be resized and sorted; the layout is remembered.
- **Search** (`Ctrl+F` or `/`) in the text, with **Whole word**; with a search active: "Result 3 of 16", move with the arrows.
- **State pills**: all, to translate, translated, approved, for proofreading, to fix, changed after proofreading, proofread.
- **More filters**: line type (dialogue / choices), kind of untranslated (in English, identical, symbols only), by check (with a problem / error / warning / does not fit the game window), file, speaker, sorting, speaker display (code / full name / both).
- **Selection**: tick lines (the **pick** column); many tools work on the picked lines.

### 2.2 The editor (detail of a line)
- **Original** and **Translation** side by side; the translation is edited straight in the `.rpy`. `Ctrl+Enter` saves and jumps to the next untranslated line, `Esc` discards. A text you changed and did not save is not lost silently (you are asked).
- **Insert buttons**: `{i}…{/i}`, `{b}`, `{w}`, `{p}`, `{nw}`, `\n`, an ellipsis, typographic quotes. A new line is written `\n`, a quote inside the text `\"`.
- **Live check** of what you type (nothing is written until you save): tags, variables, length, spelling hints, the dialogue box.
- **Approve** (`A`): the line is right as it is. **Proofread** (`P`), **flag for a fix** (`F`) with a note, **proofread everything up to here** (`Shift+P`), remove the mark (`Shift+U`).
- **Next untranslated / next unchecked / next that does not fit**; previous / next line (arrows); *Scene ◀ ▶*: the first line of the previous / next scene, the list of scenes of the file with progress.
- **Machine suggestion** for one line (a service in Settings); **Original into the field**, **Copy translation**, copy the original, copy both; **open in an external editor** (`E`).
- **Ignore** a finding on this line (it stays ignored until the text changes); **Fix from the original** where a finding has a safe fix.
- **Tabs of the detail**: *Scene* (the picture), *Context* (the original script around the line), *Memory* (other translations of the same or a similar original), *History* (the versions of the line with a diff), *Menu* (for a choice).

### 2.3 Scene preview
Draws what the player sees: follows the script back to the last `scene` and adds every picture shown since, at its place, with the text in the game's font and the dialogue box measured with it. It is an estimate from the script, not a screenshot (jumps, conditions and animations are not followed). Layered images (`layeredimage`, composites) are recognised and never drawn wrong, but not drawn yet.

### 2.4 Choice menus
A choice gets the **Menu** tab: the question (or the lines said before), every option with its translation and condition, and the first lines after each option. A choice whose text is used in several menus is marked and links to the place where its shared translation is listed. The menu is read from `.rpy` scripts (a game with only `.rpyc` files must be decompiled first).

### 2.5 Translation memory (the Memory tab)
For one line: the same original translated before in the project, and originals that differ by a few words, with their translations; a click inserts one. Free. Pro: **fill all untranslated lines** from similar translations (section 3).

### 2.6 History
Every version of every line with the time and a diff; a click puts an earlier version into the translation field (it is written only when you save).

### 2.7 Quick modes and layouts
- **Quick translating**: the scene and the editor over the whole screen, only untranslated lines. **Quick proofreading**: the same with only the lines to check; `P` marks one as checked and moves on. **Focus mode** (`Ctrl+Shift+F`): hides the top bar and the tabs. Layouts of the list and the detail can be changed (*Layout* menu) and hidden.
- A search or filter in these modes opens the first hit with a results bar (Result n of m, previous / next, a list of all hits).
- **Second window**: the detail (scene, context, tabs) in another window that follows the line open in the main one; edits stay safe (an unsaved text is never replaced).

## 3. Tools menu (Lines tab) and bulk work
Every tool shows a **preview**, writes only what you tick, and is **one undoable step**. Approved lines are never overwritten.

| Tool | What it does |
|---|---|
| **Find and replace…** | In the translated texts of all or chosen files: plain, whole word, by stem (experimental), or a regular expression (Pro). Tags and variables are protected; per-line ticks; a find on lines you picked or a file. |
| **Approve by rule…** | Approves lines by a simple rule: symbols only, interjections, names from the glossary. Goes through the normal approval (undoable). |
| **Approve all identical…** (`Shift+A`) | Approves lines whose original is the same as one you approved, with a preview. |
| **Texts the generator missed…** | Finds texts Ren'Py's generator leaves out (`show text`, input prompts, titles in variables, dialogue without a block), split into likely and maybe; tick what the player sees and it is added to `extra_strings.rpy`; ignore the rest for good; add functions of the game that show text (or *Find functions…*). Experimental. |
| **Fix…** | Safe fixes with a preview: speaker and attributes copied from the original, double spaces, typo hints. Pro (20 items in the demo). |
| **Translate with a machine…** | Suggestions in small batches (selected lines, the scene ±15, a file, everything) with progress and Stop; you edit and tick; only the ticked are written. Pro (20 lines in the demo). DeepL, LibreTranslate or an OpenAI-compatible service; tags and variables are protected. |
| **Carry over from the previous version…** | Pro: the translation of an old version of the game by the original text. |
| **Fill from similar translations…** | Pro: untranslated lines from similar originals of the same speaker in the project. |
| **Check the style…** | Pro: a language model checks the form of address, gender forms, tone and meaning. |
| **Team…** (Files) | The people who share the folder: who is in the team, their roles and files; the lead (Pro) makes the team and gives roles and files. |
| **Check the spelling…** | Pro: the lines are checked against a Hunspell dictionary of the language, with suggestions and corrections. |
| **Build a release…** | Pro: the translation as a zip for the players, with the files you attach, a README, a changelog and your own numbering. |

## 4. Check tab
One item per finding, nothing is rewritten by itself.
- **Kinds**: tags (not closed, wrong order, missing, extra, unknown, different argument), variables (`[x]` missing, extra, differing, a different count), format marks, line breaks and escapes, numbers, double spaces and repeated words, untranslated English words, glossary terms missing, length (much longer or shorter than the original), spelling hints from your own translation (missing diacritics, swapped letters), the dialogue box (text that needs more lines than the box holds, measured with the game's font).
- **Severity**: errors, warnings, and minor things (shown with a switch). Filter by kind (tags and variables / structure / text), search in the messages, per file.
- **Ignore** a finding on a line (until the text changes); the list of **ignored findings** with *Stop ignoring*.
- **Consistency**: originals translated in several different ways, with the variants; **Unify** sets one chosen translation on the lines you tick (the same words can mean something else elsewhere, so every line is ticked by you); modes: exact, distinguish the speaker, looser (words only).
- **Safe fixes** (Pro) and **Undo the last file change** for a bulk fix (restores from the backup if the files did not change since).
- **Lint**: runs the Ren'Py lint of the whole game (needs the SDK path) and shows its output, with the lines as links.
- Language packs add rules of the language (for example typographic quotes in Czech).

## 5. Glossary
- **Speakers** (the abbreviations found in the files, with the number of lines and an example) and **terms**. Give a speaker its full name; a note; a name is saved when you leave the field. Filters: all, without a name, speakers, terms; search.
- **Import from `characters.rpy`**: the names from the game's `Character(...)` definitions, a preview (new / same / conflict / dynamic), names you already have are overwritten only if you tick them.
- **Occurrences** of a term: every line whose original contains it and whether the glossary translation is used.
- **Display** of speakers in the application: code / full name / both.
- A line is checked for the glossary terms it contains. Terms found in a line are also given to the batch machine translation (a language model).
- **Profile…** per speaker (Pro): gender, how the character speaks, who they address formally; kept as a hidden last line of the note.

## 6. Game tools
The game folder is only read; work happens in a separate workspace.
- **Unpack** `.rpa` archives (built in), **decompile** `.rpyc` with unrpyc (on a copy), **generate** `game/tl/<language>` with the game's own Ren'Py engine (what `renpy translate` does; asks before writing, never overwrites an existing language folder). The "prepare a game" window is not in the interface yet; the functions work from a script and are tested on eight games.
- Characters of the game (code, name, colour) are read from the scripts and used for speaker colours and the glossary import.

## 7. Backups, snapshots, history
- **Snapshots**: zips of the `.rpy` files, made automatically (every N hours, before a bulk change) or by hand (*Create a backup now*). **Restore** (all or chosen files; the current state is saved first), delete, list of the files inside.
- **Backups of single files** are made before every write (the subfolder `backups` of the local data).
- **History** of every line (section 2.6).

## 8. Settings
| Section | What is there |
|---|---|
| **Language** | Interface language (English, Czech as a language pack); the page reloads. |
| **Project** | The translation folder; **skipped files** (names separated by commas, in all subfolders). |
| **Editor** | Which program opens a file at a line: automatic (VS Code, otherwise Notepad++), a custom command with `{file}` and `{line}`, or the default program. |
| **Translation check** | On / off; **length warnings** (longer than × original, shorter than ×, the smallest original, the minimum difference); **watch for typos**; the **dialogue box check** (characters per line, most lines in the box). |
| **Machine translation** | The service (DeepL, LibreTranslate, OpenAI-compatible incl. local), the key (plain text in `config.json`, never sent back), the address and the model, the source and target language (empty = the project's), the **consent** to send text. |
| **Keyboard shortcuts** | Every action can be rebound; `Ctrl+K` finds any action. |
| **Backups** | Automatic snapshots: on / off, interval in hours, how many to keep; the list of snapshots. |
| **Weekly goals** | Lines per week for translating and proofreading (0 = automatic). |
| **Website (publishing)** | The repository, the file in it, the name of the game on the website. Pro. |
| **Licence** | The key, activate, check now, remove from this computer, the computers that hold the licence (names, free another one), **I lost my key**. Pro. |
| **About** | The version and the state of the application, the Pro pack. |
Appearance: **automatic / dark / light**, table density.

## 9. Keyboard shortcuts
`?` shows them in the application; all can be changed in Settings.

| Keys | Action |
|---|---|
| `Ctrl+Enter` / `Esc` | in the translation field: save and go to the next untranslated line / discard |
| `Ctrl+K` | command palette (actions and files) |
| `Ctrl+F` or `/` | search in the current section |
| `Ctrl+Z` | undo the last approval, proofreading mark or glossary change |
| `F5` or `Ctrl+R` | refresh the files |
| `Ctrl+Shift+F` | focus mode |
| `F11` | full screen |
| `Alt+1`...`Alt+7` | go to a tab |
| `↓ → J` / `↑ ← K` | next / previous line |
| `PageDown` `PageUp` `Home` `End` | a page down / up, first / last line |
| `A` / `Shift+A` | approve or unapprove the line / approve all identical |
| `P` / `Shift+P` / `Shift+U` | proofread and next / proofread everything up to here / remove the mark up to here |
| `F` | flag for a fix with a note |
| `E` or `Enter` | open the line in an external editor |

Without a key by default (find them in the palette): quick translating, quick proofreading, find and replace, approve by rule, fix, the check modes, create a backup, switch appearance, switch density, a second window, change the folder.

## 10. Undo: what can be undone and how
| What | How |
|---|---|
| Approval, proofreading mark, glossary change, assignment, profile of a character | `Ctrl+Z` / the Undo button (a stack; the Overview lists the recent changes) |
| A bulk change of the files (fix, replace, machine translation, carry-over, fill, style corrections, unify) | **Undo the last file change** (menu *More* of the line editor, or the Check tab): restores the files from their backups, if they have not changed since. A bulk change also makes a snapshot first. |
| A single line you saved | the line's **History** (restore a version) or `Ctrl+Z` of your editor |
| Anything | **Snapshots**: restore the whole translation (or chosen files) from an earlier state |

## 11. Working in a team
Put the translation folder in a shared place (git, Dropbox, Syncthing). Each person's changes of state (approvals, proofreading, glossary, assignments) go to their own log in `.aside/events/`; the logs are merged when the project is opened or refreshed, so they never conflict. Split the work by files. Real team features (assignments, roles, merging edits per block) are planned (see [TEAMWORK.md](../product/TEAMWORK.md)).

## 12. The Pro features in short
| Feature | Where |
|---|---|
| Batch machine translation, safe fixes, regex replace, group suggestions, publishing | Tools menu, Files, Publishing |
| Carry a translation over to a new version, fill from similar translations, profiles of characters, style check | Tools menu, Glossary |
Details, limits of the demo and what each sends over the network: [PRO-FEATURES.md](PRO-FEATURES.md). The licence (two computers, the check every 14 days with the same time of grace, freeing computers, a lost key): [PRO-DESIGN.md](../product/PRO-DESIGN.md).

## 13. For developers
The operations of the application: [API.md](../technical/API.md) (the 80 operations of Core, generated) and [APP-ROUTES.md](../technical/APP-ROUTES.md) (what only the application around Core serves).
