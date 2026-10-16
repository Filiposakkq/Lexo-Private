# Tests on real games

Five built games (PC releases) were taken through the whole way: unpack scripts, decompile, generate translation files with the game's own engine, open the
result in Lexo. The games themselves were only read; everything was written to a workspace (`D:\RenPy Stuff\Lexo-workspace\<game>`, hard links of the
game files, so it costs almost no space). Date of the run: 2026-10-16. The games are not part of the repository.

| Game | Ren'Py | Where the scripts are | Translation files | Lines in Lexo | Time to open | Generating |
|---|---|---|---|---|---|---|
| A Wife and Mother 0.152 | 8 (Python 3.9) | `archive.rpa`: 13 `.rpy` + 13 `.rpyc` | 9 | 37 247 | 1.5 s | 19 s |
| Being a DIK 0.8.3 | 7.4.10 (Python 2.7) | `scripts.rpa`, `scripts_season2.rpa`, `gui.rpa`: 379 `.rpyc` | 72 | 45 291 | 1.4 s | 47 s |
| Eternum 0.9.5 | 8 (Python 3.9) | `archive_0.09.05.rpa`: 25 `.rpy` + 25 `.rpyc` | 16 | 73 463 | 2.3 s | 12 s |
| LoSeSb 26.8.0d | 8 (Python 3.12) | 6 archives (scripts and DLCs): 1 023 `.rpyc` | 687 | 359 753 | 13.6 s | 257 s, 2.3 GB of memory |
| Milfy City 1.0e | 7.4.11 (Python 2.7) | `scripts.rpa`: 956 `.rpyc` | 440 | 46 615 | 2.2 s | 37 s |

(The line counts leave out `common.rpy`, which holds Ren'Py's own texts and is skipped by default. The count of `translate` blocks and `old` lines in the
generated files matches Lexo's total after that.) None of the games had a `tl/<language>` folder before; the Being a DIK folder next to it holds the
author's own data of the old tool and was not touched.

## What the way looks like (and what was learned)
1. **A workspace of links** (`gametools.make_workspace`): every file of the game is a hard link, except `cache`, `saves`, `tl` and the old tools' data folders
   (not copied) and script/bytecode files (copied, because Ren'Py may rewrite them). Needs the same drive; otherwise files are copied.
2. **Scripts out of the archives** (`gametools.extract_scripts`). All five games keep their scripts in `.rpa` archives; none has loose `.rpy`.
3. **Decompile with unrpyc v2** (Python 3.9 or newer). It handled the Ren'Py 7.4 games (Python 2.7 bytecode) as well as Ren'Py 8, so the legacy unrpyc line was not
   needed in these tests. Every file that had no source was decompiled (2 358 files in the three games that needed it, no failures).
4. **Generate with the game's own engine** (`lint.find_launcher`: `<Game>.py` next to `<Game>.exe` is the same file as `renpy.py` of an SDK). No SDK is needed.
   `renpy translate` only lists loose `.rpy` files, so steps 2 and 3 are required even when the `.rpyc` files are loose.

## Problems found and what was done
| Problem | Where it showed | Fix |
|---|---|---|
| `find_python` took the Linux Python of the game's `lib` on Windows | all Ren'Py 8 games ("not a valid Win32 application") | only folders of the running platform are considered |
| `translate` produced only `common.rpy` | all games | scripts must be extracted and decompiled first (steps 2, 3) |
| unrpyc loses the indentation after a backslash inside a python block, Ren'Py rejects the file | Being a DIK `credits.rpy` | `repair_continuations` indents such lines; if a file still fails, `quarantine_unparsable` renames it to `.unparsable` and the rest is translated |
| a character called `old` made the parser see a string pair | Eternum: `old "W-WHO ARE YOU?!" with hpunch` | `old`/`new` are keywords only inside `translate <lang> strings:` |
| characters are searched in files named `characters*.rpy` only | every game except Being a DIK | when there are none, every script with a `define x = Character(...)` line counts |
| a Czech sentence hidden in an f-string | `gametools.extract_rpa` | now a message |

## Not solved / to watch
- **Characters:** in A Wife and Mother only 2 of 77 speaker codes are defined in the extracted scripts (the others are defined where the decompiled scripts do not show them);
  Lexo lists them as "missing" in the import, which is correct but may confuse.
- **Big games:** 360 000 lines open in 14 s and validation takes 3 s; generating takes minutes and a lot of memory (the game's engine initialises all of its images).
- **No UI yet** for the way above (`gametools.prepare_translation` is the function a "Prepare a game" screen would call; the menu is planned after the port).
- The line classifier (`rpy.classify`) still looks at Czech letters to tell a translated line from an English one; with other target languages it needs the language pack (step 3).
- Games with encrypted or modified archives, or with scripts that unrpyc cannot read, are not covered; the quarantine keeps the rest working.
- Only the structure of the projects was checked (opening, counts, glossary import, scene data, validation); no translation was written back in these games.
