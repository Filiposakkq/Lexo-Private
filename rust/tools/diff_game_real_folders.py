"""Compares what the Rust Core makes of real game folders (the index of pictures and fonts in loose files and in .rpa archives, the characters, the picture behind lines of the
scripts, the texts the translation misses) with what the Python Core makes of them.

    py rust/tools/diff_game_real_folders.py <folder holding games> [--exe path\\to\\lexo-golden.exe]

Every `<folder>/*/game` is read by both (the translation `game/tl/czech` or the first language folder is the translation; nothing is written). Compared: the names in the index of
the pictures, `scene_for` of every 40th line of every script, the missed texts and the suggested functions.
"""
import argparse
import glob
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path[:0] = [str(ROOT / "reference" / "python" / "core"), str(ROOT / "reference" / "python" / "core" / "tests")]

from lexo import gameimages, missed, lang   # noqa: E402
import golden_tool as gt                     # noqa: E402

SKIP = {"cache", "tl", "__pycache__", ".git"}


def python_answer(game, tl):
    lang.use(lang.NEUTRAL)
    gi = gameimages.GameImages(game).build()
    scripts = []
    for dirpath, dirs, files in os.walk(game):
        dirs[:] = [d for d in dirs if d.lower() not in SKIP]
        scripts += [Path(dirpath) / f for f in files if f.lower().endswith(".rpy")]
    scenes = {}
    for path in sorted(scripts, key=lambda p: p.relative_to(game).as_posix()):
        rel = path.relative_to(game).as_posix()
        lines = gi.script_lines(path)
        scenes[rel] = {"starts": len(gi.scene_starts(path)), "at": [[n, gi.scene_for(path, n)] for n in range(1, len(lines) + 1) if n % 40 == 0]}
    items, suggest = {"items": [], "truncated": False}, []
    if tl:
        strings, dialogue, places = missed.known_texts(tl)
        items = missed.find_missed(game, strings, dialogue, places)
        suggest = missed.suggest_functions(game, strings)
    return gt.plain({"images": {"stems": sorted(gi.stems), "paths": sorted(gi.paths), "fonts": sorted(gi.fonts), "layered": sorted(gi.layered), "composed": sorted(gi.composed),
                                "defs": gi.defs, "characters": gi.characters, "errors": gi.errors},
                     "scenes": scenes, "missed": items, "suggest": suggest})


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("games")
    ap.add_argument("--exe", default=os.environ.get("LEXO_GOLDEN_EXE", "lexo-golden"))
    a = ap.parse_args()
    games = sorted(d for d in glob.glob(os.path.join(a.games, "*", "game")) if os.path.isdir(d))
    print(len(games), "game folders")
    bad = 0
    for g in games:
        langs = sorted(d for d in glob.glob(os.path.join(g, "tl", "*")) if os.path.isdir(d) and glob.glob(os.path.join(d, "**", "*.rpy"), recursive=True))
        tl = next((d for d in langs if d.lower().endswith("czech")), langs[0] if langs else None)
        out = subprocess.run([a.exe, "--dump-game", g, *([tl] if tl else [])], capture_output=True, check=True).stdout.decode("utf-8")
        got, want = json.loads(out), python_answer(g, tl)
        if got != want:
            bad += 1
            for part in want:
                if got.get(part) != want[part]:
                    print("DIFFERENT:", g, "->", part)
        else:
            print("same:", g, "-", len(want["images"]["stems"]), "pictures,", len(want["missed"]["items"]), "missed texts")
    print(f"{len(games)} compared, {bad} different")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
