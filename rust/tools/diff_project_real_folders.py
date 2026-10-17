"""Compares what the Rust Core makes of real translation folders (discovery, parsing, counts, proofread marks) with what the Python Core makes of them.

    py rust/tools/diff_project_real_folders.py <folder holding games> [--exe path\\to\\lexo-golden.exe]

Every folder `<folder>/*/game/tl/<language>` is opened by both (the store is read from its `.lexo` folder, `common.rpy` is skipped, the neutral language pack is used; nothing is written)
and the files found, the rows of the overview and the proofread counts of every file are compared.
"""
import argparse
import glob
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path[:0] = [str(ROOT / "core"), str(ROOT / "core" / "tests")]

from lexo import layout, project, store    # noqa: E402
import golden_tool as gt                   # noqa: E402


def python_answer(folder):
    folder = Path(folder)
    lay = layout.Layout()
    try:
        lay = layout.parse(json.loads((folder / ".lexo" / "lexo.json").read_text(encoding="utf-8-sig")))
    except (OSError, ValueError):
        pass
    layout.use(lay)
    layout.use(layout.Layout(lay.sections, lay.labels, "", lay.series, lay.folders))        # the neutral pack, like the Rust side
    st = store.Store(folder / ".lexo").load()
    p = project.Project(folder, st, skip=["common.rpy"])
    report = p.refresh()
    rows = p.rows()
    files = sorted(p.files)
    return gt.plain({"added": report.added, "empty": report.empty, "errors": len(report.errors), "rows": [list(r) for r in rows],
                     "proof": {f: [*p.proof_counts(f), p.proof_stale(f)] for f in files}})


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("games")
    ap.add_argument("--exe", default=os.environ.get("LEXO_GOLDEN_EXE", "lexo-golden"))
    a = ap.parse_args()
    folders = sorted(d for d in glob.glob(os.path.join(a.games, "*", "game", "tl", "*")) if os.path.isdir(d))
    print(len(folders), "translation folders")
    out = subprocess.run([a.exe, "--dump-project", *folders], capture_output=True, check=True).stdout.decode("utf-8")
    rows = dict(line.split("\t", 1) for line in out.splitlines() if "\t" in line)
    bad = 0
    for f in folders:
        got, want = json.loads(rows.get(f, "null")), python_answer(f)
        if got != want:
            bad += 1
            for part in want:
                if got is None or got.get(part) != want[part]:
                    print("DIFFERENT:", f, "->", part)
    print(f"{len(folders)} compared, {bad} different")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
