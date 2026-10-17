#!/usr/bin/env python3
"""Run the tests of Core (`core/tests`) and of Free (`free/tests`).

    py run_tests.py              all suites, one after the other (pro only where the folder pro/ has tests: the private monorepo)
    py run_tests.py core         only Core (no UI, no HTTP server: the library alone)
    py run_tests.py free         only Free (the server, the UI files, the command line; it needs Core)
    py run_tests.py pro          only the Pro pack (it needs Core; Core and Free never need it)
    py run_tests.py core -v      anything after the suite name goes to `unittest` (-v, -k name, ...)

Core is on the import path of both suites; Free is not on the path of the Core suite, which is how the tests prove that Core does not need it.
"""
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SUITES = {
    "core": (ROOT / "core" / "tests", [ROOT / "core", ROOT / "core" / "tests"]),
    "free": (ROOT / "free" / "tests", [ROOT / "core", ROOT / "core" / "tests", ROOT / "free"]),
    "pro": (ROOT / "pro" / "tests", [ROOT / "core", ROOT / "core" / "tests", ROOT / "pro"]),
}


def run(name, extra):
    start, paths = SUITES[name]
    env = dict(os.environ, PYTHONPATH=os.pathsep.join(str(p) for p in paths), PYTHONIOENCODING="utf-8")
    print(f"== {name}: {start.relative_to(ROOT)}", flush=True)
    return subprocess.call([sys.executable, "-m", "unittest", "discover", "-s", str(start), *extra], cwd=ROOT, env=env)


def main(argv):
    names = [a for a in argv[:1] if a in SUITES]
    extra = argv[1:] if names else argv
    failed = [n for n in (names or [s for s, (start, _p) in SUITES.items() if start.is_dir()]) if run(n, extra)]
    if failed:
        print("FAILED:", ", ".join(failed))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
