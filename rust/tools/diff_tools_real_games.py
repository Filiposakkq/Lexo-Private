"""Compares what the Rust Core's game tools make of real built games (the index of every .rpa archive, the scripts inside it, the workspace of hard links, the copied scripts) with what
the Python Core makes of them.

    py rust/tools/diff_tools_real_games.py <folder holding games> [--exe path\\to\\lexo-golden.exe] [--scratch folder]

Every `<folder>/*/game` is read by both (nothing is written to the game; the archives' scripts, the workspace and the copied scripts are made in the scratch folder, which must be on the same
drive as the games so that links work, and removed again). Compared: the index of every archive, the SHA-1 of every script unpacked from it, the counts and the files (path and size) of the
workspace, the SHA-1 of the copied scripts.
"""
import argparse
import glob
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path[:0] = [str(ROOT / "core"), str(ROOT / "core" / "tests")]

from lexo import gametools, rpa   # noqa: E402
from lexo.messages import text_of  # noqa: E402
import golden_tool as gt           # noqa: E402

SCRIPTS = (".rpy", ".rpyc", ".rpym", ".rpymc")


def tree(dir_, sizes):
    out = {}
    for f in sorted(Path(dir_).rglob("*")):
        if f.is_file():
            out[f.relative_to(dir_).as_posix()] = f.stat().st_size if sizes else hashlib.sha1(f.read_bytes()).hexdigest()
    return out


def python_answer(root, scratch):
    game = Path(root) / "game"
    archives = {}
    for i, arc in enumerate(gametools.find_archives(game)):
        rel = arc.relative_to(game).as_posix()
        try:
            index = [list(x) for x in gametools.list_archive(arc)]
        except (gametools.ToolError, rpa.RpaError) as e:
            archives[rel] = {"error": text_of(e)}
            continue
        dest = scratch / f"x{i}"
        try:
            res = gametools.extract_rpa(arc, dest, only=lambda n: n.lower().endswith(SCRIPTS))
            got = {"extract": res, "files": tree(dest, False)}
        except (gametools.ToolError, rpa.RpaError) as e:
            got = {"error": text_of(e)}
        shutil.rmtree(dest, ignore_errors=True)
        archives[rel] = {"index": index, "scripts": got}
    ws = scratch / "ws"
    try:
        workspace = {"counts": gametools.make_workspace(root, ws), "files": tree(ws, True)}
    except (gametools.ToolError, rpa.RpaError) as e:
        workspace = {"error": text_of(e)}
    shutil.rmtree(ws, ignore_errors=True)
    scripts = scratch / "scripts"
    try:
        copied = {"count": gametools.copy_scripts(game, scripts), "files": tree(scripts, False)}
    except (gametools.ToolError, rpa.RpaError) as e:
        copied = {"error": text_of(e)}
    shutil.rmtree(scripts, ignore_errors=True)
    return gt.plain({"archives": archives, "workspace": workspace, "copy_scripts": copied})


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("games")
    ap.add_argument("--exe", default=os.environ.get("LEXO_GOLDEN_EXE", "lexo-golden"))
    ap.add_argument("--scratch", default=None)
    a = ap.parse_args()
    scratch_root = Path(a.scratch) if a.scratch else Path(a.games).resolve().parent / "lexo-tools-scratch"
    games = sorted(os.path.dirname(d) for d in glob.glob(os.path.join(a.games, "*", "game")) if os.path.isdir(d))
    print(len(games), "games")
    bad = 0
    for g in games:
        scratch = scratch_root / Path(g).name
        shutil.rmtree(scratch, ignore_errors=True)
        scratch_py, scratch_rs = scratch / "py", scratch / "rs"
        scratch_py.mkdir(parents=True)
        scratch_rs.mkdir(parents=True)
        out = subprocess.run([a.exe, "--dump-tools", g, str(scratch_rs)], capture_output=True, check=True).stdout.decode("utf-8")
        got, want = json.loads(out), python_answer(g, scratch_py)
        shutil.rmtree(scratch, ignore_errors=True)
        if got != want:
            bad += 1
            for part in want:
                if got.get(part) != want[part]:
                    print("DIFFERENT:", g, "->", part)
        else:
            files = len(want["workspace"].get("files", {}))
            print("same:", Path(g).name, "-", len(want["archives"]), "archives,", files, "files in the workspace,", want["copy_scripts"].get("count"), "scripts copied")
    shutil.rmtree(scratch_root, ignore_errors=True)
    print(f"{len(games)} compared, {bad} different")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
