"""Milestone M9: runs the operations of the Python Core and of the Rust Core on copies of the translations of real games and compares their answers.

    py rust/tools/diff_ops_real_games.py <folder holding games> [--exe path\\to\\lexo-golden.exe] [--scratch folder] [--lang czech] [--games name,name] [--keep]

For every `<folder>/*/game/tl/<lang>` the program makes TWO copies in the scratch folder (the translation with its `.aside` data, and the scripts of the game without the big files), opens
one with the Python Core and one with the Rust Core, asks both the same list of read operations (made from what the Python Core answers: the files, a sample of lines of every file)
and compares the normalised answers. The originals are only read. The scratch folder should be on a fast drive; it is removed at the end (`--keep` leaves it).

Exit code 0 = no difference.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path[:0] = [str(ROOT / "core"), str(ROOT / "core" / "tests")]

import golden_tool as gt   # noqa: E402

SAMPLE_PER_FILE = 6
MAX_SAMPLED_FILES = 25
SCRIPT_SUFFIXES = (".rpy", ".rpym")
SKIP_DIRS = {"tl", "cache", "saves", "__pycache__", ".git"}

FIXED = [
    ("state", {}), ("ui-langs", {}), ("features", {}), ("settings", {}), ("overview", {}), ("files", {}), ("ignore/list", {}), ("glossary", {}), ("groups/suggest", {}),
    ("consistency", {}), ("consistency", {"loose": True}), ("consistency", {"by_speaker": True}), ("missed", {}), ("missed", {"level": "all"}), ("missed/functions", {}),
    ("game/characters", {}), ("snapshots", {}), ("lint/status", {}), ("export/preview", {}), ("proof/next", {}),
    ("validation", {"limit": 500, "group": "line"}), ("validation", {"limit": 500}), ("validation", {"severity": "error", "limit": 300}), ("validation", {"ignored": "all", "limit": 300}),
    ("validation", {"category": "length", "limit": 200}), ("entries", {"limit": 500, "status": "todo"}), ("entries", {"limit": 500, "issues": "any"}),
    ("entries", {"limit": 500, "sort": "kind"}), ("entries", {"limit": 500, "sort": "issues"}), ("entries", {"limit": 200, "status": "review", "q": "the"}),
]


def copy_game(game, lang, dst):
    """<dst>/game/tl/<lang> (everything, with `.aside`) and the scripts of the game around it. Returns the folder to open."""
    src_game = Path(game) / "game"
    for dirpath, dirs, files in os.walk(src_game):
        dirs[:] = [d for d in dirs if d.lower() not in SKIP_DIRS]
        for f in files:
            if f.lower().endswith(SCRIPT_SUFFIXES):
                target = Path(dst) / "game" / Path(dirpath).relative_to(src_game) / f
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(Path(dirpath) / f, target)
    tl = Path(dst) / "game" / "tl" / lang
    shutil.copytree(src_game / "tl" / lang, tl)
    return tl


def open_python(tl, cfg):
    os.environ["LEXO_CONFIG_DIR"] = str(cfg)
    from lexo.app import App
    from lexo.config import Config
    app = App(Config().load())
    app.open_project(tl)
    return app


def make_ops(app):
    """The list of [operation, parameters]: the fixed reads, the pages of every file and a sample of lines with the operations that look at one line."""
    from lexo import api
    ops = [list(x) for x in FIXED]
    files = [f["name"] for f in api.call(app, "files")["files"]]
    for rel in files[:MAX_SAMPLED_FILES]:
        ops.append(["entries", {"file": rel, "limit": 500}])
        ops.append(["validation", {"file": rel, "limit": 200}])
        ops.append(["scenes", {"file": rel, "line": 0}])
        res = api.call(app, "entries", {"file": rel, "limit": 500})
        items = res["items"]
        step = max(1, len(items) // SAMPLE_PER_FILE)
        for it in items[::step][:SAMPLE_PER_FILE]:
            p = {"file": rel, "line": it["line"]}
            for op in ("entry", "entry/context", "entry/scene", "entry/menu", "entry/memory", "entry/history", "game/scene"):
                ops.append([op, dict(p)])
            ops.append(["entry/check", {**p, "text": it["target"] or "Ahoj."}])
        for kind in ("todo", "review", "fix", "over"):
            ops.append(["entry/next", {"file": rel, "line": 0, "kind": kind}])
        ops.append(["proof/next", {"file": rel}])
    return ops


def run_python(app, ops):
    from lexo import api
    from lexo.app_base import AppError
    out = []
    for name, params in ops:
        try:
            out.append({"answer": gt.plain(api.call(app, name, gt.native(params)))})
        except AppError as e:
            out.append({"error": str(gt.text_of(e))})
        except KeyError as e:
            out.append({"error": f"'{e.args[0]}'"})
    return out


def first_differences(a, b, path="$", limit=3):
    if type(a) is not type(b) and not (isinstance(a, (int, float)) and isinstance(b, (int, float)) and not isinstance(a, bool) and not isinstance(b, bool)):
        return [f"{path}: {json.dumps(a, ensure_ascii=False)[:80]} / {json.dumps(b, ensure_ascii=False)[:80]}"]
    if isinstance(a, dict):
        out = []
        for k in sorted(set(a) | set(b)):
            if k not in a or k not in b:
                out.append(f"{path}.{k}: only in {'python' if k in a else 'rust'}")
            else:
                out += first_differences(a[k], b[k], f"{path}.{k}", limit)
            if len(out) >= limit:
                break
        return out[:limit]
    if isinstance(a, list):
        if len(a) != len(b):
            return [f"{path}: {len(a)} items / {len(b)} items"]
        out = []
        for i, (x, y) in enumerate(zip(a, b)):
            out += first_differences(x, y, f"{path}[{i}]", limit)
            if len(out) >= limit:
                break
        return out[:limit]
    if a != b:
        return [f"{path}: {json.dumps(a, ensure_ascii=False)[:100]} / {json.dumps(b, ensure_ascii=False)[:100]}"]
    return []


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("games")
    ap.add_argument("--exe", default=os.environ.get("LEXO_GOLDEN_EXE", "lexo-golden"))
    ap.add_argument("--scratch", default=None)
    ap.add_argument("--lang", default="czech")
    ap.add_argument("--games-only", default="")
    ap.add_argument("--keep", action="store_true")
    a = ap.parse_args()
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    scratch_root = Path(a.scratch) if a.scratch else Path(tempfile.mkdtemp(prefix="lexo-m9-"))
    scratch_root.mkdir(parents=True, exist_ok=True)
    wanted = [w.strip().lower() for w in a.games_only.split(",") if w.strip()]
    games = sorted(p for p in Path(a.games).iterdir() if (p / "game" / "tl" / a.lang).is_dir() and (not wanted or any(w in p.name.lower() for w in wanted)))
    print(len(games), "games with game/tl/" + a.lang)
    bad = 0
    for g in games:
        base = scratch_root / g.name
        shutil.rmtree(base, ignore_errors=True)
        py_dir, rs_dir = base / "py", base / "rs"
        tl_py, tl_rs = copy_game(g, a.lang, py_dir), copy_game(g, a.lang, rs_dir)
        app = open_python(tl_py, py_dir / "cfg")
        try:
            ops = make_ops(app)
            want = run_python(app, ops)
        finally:
            app.close()
        ops_file = base / "ops.json"
        ops_file.write_text(json.dumps(ops, ensure_ascii=False), encoding="utf-8")
        # each Core keeps its settings and local data in "<copy>/cfg" (the Rust one is given the folder of its copy)
        proc = subprocess.run([a.exe, "--run-ops", str(tl_rs), str(rs_dir), str(ops_file)], capture_output=True, check=True)
        got = json.loads(proc.stdout.decode("utf-8"))
        roots_py, roots_rs = gt._roots(py_dir), gt._roots(rs_dir)
        got = [gt.normalize(x, roots_rs) for x in got]
        want = [gt.normalize(x, roots_py) for x in want]
        diffs = {}
        for (name, params), w, r in zip(ops, want, got):
            d = first_differences(w, r)
            if d:
                diffs.setdefault(name, []).append((params, d))
        if len(want) != len(got):
            diffs.setdefault("(count)", []).append(({}, [f"{len(want)} answers / {len(got)} answers"]))
        if diffs:
            bad += 1
            print(f"DIFFERENT: {g.name}: {sum(len(v) for v in diffs.values())} of {len(ops)} calls")
            for name, items in sorted(diffs.items()):
                params, d = items[0]
                print(f"   {name}: {len(items)} calls differ; first {json.dumps(params, ensure_ascii=False)[:120]}")
                for line in d:
                    print("       ", line)
        else:
            errors = sum(1 for x in want if "error" in x)
            print(f"same: {g.name}: {len(ops)} calls ({errors} of them answered with an error, the same in both)")
        if not a.keep:
            shutil.rmtree(base, ignore_errors=True)
    if not a.keep and not a.scratch:
        shutil.rmtree(scratch_root, ignore_errors=True)
    print(f"{len(games)} compared, {bad} different")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
