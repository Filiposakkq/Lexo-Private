# Game tools (preparing a game for translating)

Code: `free/lexo/gametools.py`. Goal: from a game folder without a translation to a `game/tl/<language>` skeleton that the app opens.

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

## Texts the generator misses (`missed.py`, Tools > "Texts the generator missed…")
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
