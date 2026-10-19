"""Compares the parser of Rust Core with the one of Python Core on real translation files (the first step of M9 in docs/technical/RUST-PORT.md).

    py rust/tools/diff_rpy_real_files.py <folder holding games> [--exe path\\to\\lexo-golden.exe] [--lang cs]

Every `*.rpy` under `<folder>/*/game/tl/*/` is parsed by both and the answers (the same shape as the golden data) must be equal. The files are only read.
Build the Rust side first: `cargo build -p lexo-golden` (the exe is in the target folder).
"""
import argparse
import glob
import json
import os
import subprocess
import sys
from dataclasses import asdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path[:0] = [str(ROOT / "reference" / "python" / "core"), str(ROOT / "reference" / "python" / "core" / "tests")]

from lexo import lang, rpy, spelling, validator   # noqa: E402
import golden_tool as gt     # noqa: E402


def python_answer(path, pack):
    lang.use(pack)
    parsed = rpy.parse_bytes(Path(path).read_bytes())
    return gt.plain({
        "entries": [{**asdict(e), "pending": e.pending, "key": list(e.key)} for e in parsed.entries],
        "block_ids": [list(b) for b in parsed.block_ids], "structure": [list(s) for s in parsed.structure],
        "digest": parsed.digest, "languages": sorted(parsed.languages), "counts": list(rpy.count(parsed.entries))})


def python_checks(path, pack, spell=None):
    lang.use(pack)
    parsed = rpy.parse_bytes(Path(path).read_bytes())
    return gt.plain([[line, issue.to_dict()] for line, issue, _entry in validator.check_parsed(parsed, spell=spell)])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("games")
    ap.add_argument("--exe", default=os.environ.get("LEXO_GOLDEN_EXE", "lexo-golden"))
    ap.add_argument("--checks", action="store_true", help="compare the issues of the checks (validator) instead of the parse")
    ap.add_argument("--spell", action="store_true", help="with --checks: also the spelling hints (the vocabulary is the translated text of each chunk of 100 files)")
    ap.add_argument("--lang", default="cs", choices=sorted(gt.LANGS))
    a = ap.parse_args()
    files = sorted(glob.glob(os.path.join(a.games, "*", "game", "tl", "*", "**", "*.rpy"), recursive=True))
    print(len(files), "files")
    bad = 0
    for i in range(0, len(files), 100):                     # in chunks: the command line of Windows is limited
        chunk = files[i:i + 100]
        flag = "--dump-spell" if a.checks and a.spell else "--dump-check" if a.checks else "--dump"
        spell = None
        if a.checks and a.spell:
            lang.use(gt.LANGS[a.lang])
            parsed = [rpy.parse_bytes(Path(f).read_bytes()) for f in chunk]
            spell = spelling.Spelling.build([e.target for p in parsed for e in p.entries if not e.pending])
        out = subprocess.run([a.exe, flag, a.lang, *chunk], capture_output=True, check=True).stdout.decode("utf-8")
        rows = dict(line.split("\t", 1) for line in out.splitlines() if "\t" in line)
        for f in chunk:
            if json.loads(rows.get(f, "null")) != (python_checks(f, gt.LANGS[a.lang], spell) if a.checks else python_answer(f, gt.LANGS[a.lang])):
                bad += 1
                if bad <= 10:
                    print("DIFFERENT:", f)
    print(f"{len(files)} compared, {bad} different")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
