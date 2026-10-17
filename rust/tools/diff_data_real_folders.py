"""Compares what the Rust Core reads from real data folders (`.lexo`: the CSV files of the store, history.csv, the event log) with what the Python Core reads.

    py rust/tools/diff_data_real_folders.py <folder holding games> [--exe path\\to\\lexo-golden.exe] [--today 2026-10-17]

Every folder named `.lexo` under `<folder>/*/game/` is read by both (nothing is written: the store is only loaded, the event files only read) and the answers are compared:
the store, the rows and points of history.csv, the completion estimate and goals for `today`, and the replay of the events.
"""
import argparse
import glob
import json
import os
import subprocess
import sys
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path[:0] = [str(ROOT / "core"), str(ROOT / "core" / "tests")]

from lexo import history, layout, store    # noqa: E402
from lexo.events import EventLog           # noqa: E402
import golden_tool as gt                   # noqa: E402


def python_answer(folder, today):
    folder = Path(folder)
    lay = layout.Layout()
    try:
        lay = layout.parse(json.loads((folder / "lexo.json").read_text(encoding="utf-8-sig")))
    except (OSError, ValueError):
        pass
    layout.use(lay)
    st = store.Store(folder).load()
    dump = {
        "corr": st.corr, "assign": st.assign, "approved": {f: sorted([list(k) for k in v]) for f, v in st.approved.items()},
        "episodes": {str(n): s for n, s in st.episodes.items()},
        "proof": {f: [[b, o, dict(m)] for (b, o), m in sorted(t.items())] for f, t in st.proof.items()},
        "ignored": {f: [[b, o, c, dict(m)] for (b, o, c), m in sorted(t.items())] for f, t in st.ignored.items()},
        "glossary": st.glossary, "warnings": list(st.warnings), "unsaved": [], "undo": []}
    h = history.History(folder / "history.csv")
    points = h.points()
    done, total = (points[-1][1], points[-1][2]) if points else (0, 0)

    def dates(v):
        if isinstance(v, date):
            return v.isoformat()
        if isinstance(v, dict):
            return {k: dates(x) for k, x in v.items()}
        return v
    log = EventLog(folder / "events", folder / "no-seen-file.json", "nobody")
    seen, unseen, _ends = log.read_all()
    log.replay(seen)
    log.replay(unseen)
    flat = {kind: [[list(key), v, list(stamp)] for key, (v, stamp) in slot.items() if v is not None] for kind, slot in log.state.items()}
    return gt.plain({
        "store": dump,
        "history": {"rows": h.rows(), "points": [[str(ts), d, t, c] for ts, d, t, c in points], "chart": [[str(ts), x, y] for ts, x, y in h.chart_points()],
                    "estimate": dates(history.estimate(points, done, total, today=today)),
                    "goals": dates(history.goals(points, max(0, total - done), max(0, total - done), None, today))},
        "events": flat})


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("games")
    ap.add_argument("--exe", default=os.environ.get("LEXO_GOLDEN_EXE", "lexo-golden"))
    ap.add_argument("--today", default="2026-10-17")
    a = ap.parse_args()
    folders = sorted(glob.glob(os.path.join(a.games, "*", "game", "**", ".lexo"), recursive=True))
    print(len(folders), "data folders")
    out = subprocess.run([a.exe, "--dump-data", a.today, *folders], capture_output=True, check=True).stdout.decode("utf-8")
    rows = dict(line.split("\t", 1) for line in out.splitlines() if "\t" in line)
    bad = 0
    for f in folders:
        got, want = json.loads(rows.get(f, "null")), python_answer(f, date.fromisoformat(a.today))
        if got != want:
            bad += 1
            for part in want:
                if got and got.get(part) != want[part]:
                    print("DIFFERENT:", f, "->", part)
    print(f"{len(folders)} compared, {bad} different")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
