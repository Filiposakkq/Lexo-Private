# Game tools (preparing a game for translating)

Code: `core/lexo/gametools.py`. Goal: from a game folder without a translation to a `game/tl/<language>` skeleton that the app opens.

| Step | What | How |
|---|---|---|
| 1. Unpack `.rpa` archives | scripts (`.rpyc`) and images are often packed | native (`extract_rpa`), stays in the app also after the Rust port |
| 2. Decompile `.rpyc` | the SDK needs script sources | **unrpyc as an external helper** (MIT licence), run on a **copy** of the scripts in a workspace folder |
| 3. Generate translation files | what `renpy <game> translate <language>` does | **the Ren'Py SDK itself** (`generate_translation`), not re-implemented |

## Rules
- The game folder is only read. Unpacked and decompiled files go to a workspace folder; `copy_scripts` refuses a workspace inside the game.
- Step 3 writes `game/tl/<language>` into the project it is run for, so it needs an explicit confirmation (`write_into_project=True`) and never touches an existing language folder. The language name is checked (letters, digits, underscore) before it reaches a command line.
- Archive entries cannot leave the target folder (`..`, absolute paths, drive letters); one file and the total size are limited; existing files are kept unless asked.
- External programs run without a shell, with a time limit, and their output is shown to the user.

## unrpyc versions
v2 needs Python 3.9+ and is for Ren'Py 8 games; v1 (legacy branch) runs on the Python 2.7 of a Ren'Py 7 SDK. The caller chooses the script and the Python
(`lint.find_python(sdk)` gives the one that ships with an SDK). Later the app can bundle unrpyc (MIT: keep the licence text and the authors).

## Not done yet
UI and server routes for these steps (they come with the new UI and the project menu), choosing unrpyc v1/v2 automatically from the detected Ren'Py version, a real unrpyc run in CI.

## From a built game to translation files (tested on five games, see GAME-TESTS.md)
`gametools.prepare_translation(game_root, workspace, language, unrpyc_python, unrpyc_script)` runs the whole way in a workspace:
1. `make_workspace`: hard links of the game (not `cache`, `saves`, `tl`, `_progress_data`, `.lexo`); script and bytecode files are copied;
2. `extract_scripts`: the scripts inside the `.rpa` archives are unpacked next to them;
3. `decompile` (unrpyc), then `repair_decompiled` (indentation after a backslash in python blocks);
4. `generate_translation` with the game's own engine (`lint.find_launcher`); files named in Ren'Py's error output are renamed to `.unparsable` and the run is repeated (up to three times).
The game folder is only read. No UI yet.

## Texts the generator misses (`missed.py`, Tools > "Texts the generator missed…") - experimental, Free
The whole feature is marked **experimental**: it is a heuristic and can be wrong in both directions (a text it does not find, a text it lists that the player never sees). It works to a good degree and saves a lot of searching, and it is part of the Free version, but the list must always be reviewed.
`renpy translate` writes blocks for dialogue, menu choices, screens and strings marked `_("...")`. A text shown another way has no block: `show text "..."`,
`renpy.input("prompt")`, `renpy.notify(...)`, a title assigned to a variable and shown later with `[title]`, and dialogue in places the generator does not reach.
Such texts can still be translated with a `translate <language> strings:` block (old / new).
Lexo reads the game's scripts (the decompiled `.rpy`), compares them with the translation and lists what it does not have:
- `likely`: the text goes to `show text`, `renpy.input`, `renpy.notify`, `Text(...)`..., or is assigned to a variable whose name says text (title, name, message...), or it is a dialogue line at a place that no translation block points to (`# game/script.rpy:109` comments);
- `maybe`: any other string that looks like a sentence (shown only on request, a long list).
Menu choices, screen language (`text "..."` in screens), image/transform/style blocks, `Character(...)` definitions and `config`/`build`/`gui` settings are skipped.
The chosen texts are appended to `<translation folder>/extra_strings.rpy` (untranslated, so they show up as lines to do); texts already in the string table are never written twice.
Checked on Being a DIK: of the 27 strings the translator had added by hand, 24 are found (all as `likely`); the other 3 are not missed texts (the generator did extract two
of them, one is built from pieces in the code). The list has 185 likely texts for that game, mostly more `show text` lines the translator had not reached yet.
It is a heuristic: review the list, untick what the player never sees (a build name, a file label).

### The project's own functions and ignored texts (`.lexo/missed.json`)
- **Functions that show text:** a game may show text through its own function (`notify("Item added")`, `show_message(...)`, `Item("Spy Camera")`). The dialog has a field for them (comma separated)
  and "Find functions…", which lists the functions of the game that are called again and again with a text as their first argument (at least three times; methods of objects and lists, exceptions, `renpy.*` and
  similar are left out). A click adds a name. Texts passed to these functions become `likely`.
- **Ignored texts:** "Ignore selected" puts texts that the player never sees (a build name, a label) into the project's ignore list; they stay out of every later search until "Forget the ignored texts".
- The file is `<translation folder>/.lexo/missed.json`, shared like the other project files. Built-in: any `<name>.say("...")` call counts as a text sink.

### Where the added texts go
The place does not matter to the game: Ren'Py reads every `translate <language> strings:` block of every file under `tl/<language>`. **By default all the texts go to `extra_strings.rpy`**, one tidy place.
The dialog has "Select all / Select none" and an option **"Put each text into the translation file of its script" (experimental)**: the text is then written at the end of the file that mirrors its own script
(`script2.rpy` -> `tl/czech/script2.rpy`) as one more strings block, so the file and group statistics follow it; a text whose script has no translation file still goes to `extra_strings.rpy`.
Experimental means: it mostly works and saves work, but it edits your translation files and may not do the right thing in every game; a snapshot is made first. Line endings are kept, and a path that would lead outside
the translation folder is never followed. The feature is part of the Free version.

## Scene preview and layered images (not supported yet, planned)
The preview follows the game script upwards to the last `scene` (the background) and draws the pictures `show`n over it since (a `show` with the same tag replaces the older one in place, `hide` removes it, a new `scene` clears the screen). A shown picture keeps its natural size and stands on the bottom edge; its side comes from the `at` clause (`left`, `center`, `right`; a transform with left / right in its name is taken as such, any other transform as the centre). Jumps, conditions and ATL movements are not followed. That is exactly right for games made of full-screen renders and wrong for games that build a
picture from several parts. **Layered images are not supported yet; support is planned.** What the preview does instead of a wrong picture:
- **Recognised as layered or composed:** `layeredimage name:`, `image x = Composite(...)` / `LiveComposite` / `im.Composite` / `ConditionSwitch` / `DynamicImage` / `Fixed`, and ATL blocks with `contains`.
  Their parts are never taken for the whole picture.
- **A layered character on a background:** the background is shown, and a note says that characters made of layers are not drawn.
- **A scene that is itself layered:** no picture is shown (not an older, wrong one), and the preview says why.
Measured on eight games: the three that preview badly use them heavily (LoSeSb: 998 `layeredimage` names, even for backgrounds such as `bg alley`; Our Red String about 500 and Four Elements about 6 600 composed images); the others have almost none. In a sample of 250 lines per game the preview shows: LoSeSb a layered scene without picture in 170, a background with the note in 58; Our Red String 154 with the note and 15 without picture; Four Elements 130 and 14; Eternum, Being a DIK, Milfy City, A Wife and Mother and Midnight Paradise at most 3 lines without a picture and no note.
Drawing them would need the definitions evaluated: reading the attribute groups and `if` conditions of `layeredimage`, and the code of composites and switches. That is closest to running the game's engine for the picture, so it belongs after the Rust port or with an engine-backed renderer.

### How exact the preview is
The preview is an estimate from the game script, not a screenshot. For games with layered images (characters or backgrounds built from several parts, animations, conditions) it can be wrong or incomplete, and it may not be a good picture of the scene; the app says so under the preview. The font of the text is taken from the game when it can be found (otherwise a substitute is used);
the frames of the text box and the name box (their pictures, colours and shapes) are not modelled and can look different in the game. The app says this under the preview.

## Choices and their menu
A choice (an entry of `translate <lang> strings:` whose game script position is the line of a `"Text":` inside a `menu:`) gets a **Menu** tab in the line detail (it is the default tab for choices). `menus.py` reads the script text:
- the **question**: a line of dialogue inside the menu before the first choice; a menu without one shows the last two lines said before it (many games have no question);
- every **choice** with its translation (found by the original text), and `if condition` when it has one;
- the first two lines of dialogue **after each choice** (the body of the choice), with original and translation, because the choice is read together with what follows it; a menu inside a choice is not mixed into it.
The same tab also appears for a **line of dialogue that belongs to a menu**: its question, the first lines of a choice, or the two lines said just before the menu; that line is marked "this line". The lines before a menu are taken from the story path to it: a menu that opens an `if` / `else` block looks back past the `if`, and the bodies of earlier branches or earlier menus are skipped.
A choice that is followed by no dialogue (many only set a variable and `jump`) shows the first four lines of its body instead, so the translator sees what it does. The menu is also shown under the film strip in the quick translating and quick proofreading modes (only for lines that belong to a menu).
Limits: the script must be available as `.rpy` (not only `.rpyc`); jumps, `call screen` and menus built in Python are not followed; a text used in several menus shows only the first place Ren'Py recorded for it. Strings that are interface text (not a menu choice) say so.
Ren'Py 7 records the line of the `menu:` statement as the position of every choice of that menu (Being a DIK); newer versions record the line of the choice itself. Both work: with the menu line, the choice is told apart by its original text. Checked on the script of Being a DIK: all 92 menus are found.

