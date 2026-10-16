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
