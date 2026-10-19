# unrpyc (a part of the application, not changed)

[unrpyc](https://github.com/CensoredUsername/unrpyc) decompiles the compiled scripts of a Ren'Py game (`.rpyc`) into text (`.rpy`). Aside uses it when it prepares a game (Home, *Prepare a game*), only when the game has scripts that exist as bytecode alone.

| Folder | Version | Runs on | For games made with |
|---|---|---|---|
| `v2/` | 2.0.4 | the Python that ships with the game, 3.9 or later | Ren'Py 8 |
| `v1/` | 1.3.2 | the Python that ships with the game, 2.7 | Ren'Py 7 and older |

Aside picks the folder by the Python of the game (`lib/py3-…` → `v2`, otherwise `v1`) and never uses the Python of the user's computer.

- **Licence:** MIT, Copyright (c) 2012-2024 Yuri K. Schlesner, CensoredUsername, Jackmcbarn. The text is in `v1/LICENSE` and `v2/LICENSE`; it stays with the files, as the licence asks, and the application ships it.
- **Source:** the release archives `v1.3.2` and `v2.0.4` of <https://github.com/CensoredUsername/unrpyc>. Only what runs is kept (`unrpyc.py`, `deobfuscate.py`, the `decompiler` package); the tests, the in-game helper and the packaging files are left out. The files are **not modified**: the two folders are the files of the releases.
- **To update:** download the new release, replace the folder (and the version in this table and in `docs/technical/GAME-TOOLS.md`), and run the tests of the preparation.
