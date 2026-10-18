"""Runs the UI self-test of Free (free/lexo_free/ui/selftest.js, 139 checks) against the RUST server, in a real browser, headless.

    py rust/tools/run_ui_selftest.py [--exe path\to\lexo-server.exe]

It starts `lexo-server --selftest` (a synthetic project in a temporary folder, no Pro pack) and then does what `free/tests/ui/run_selftest_ci.py` does against the Python server, so the two
are checked by the same code. Needs Playwright (`py -m pip install playwright`) and Microsoft Edge (set BADIK_BROWSER=chrome for Chrome, or "" for Playwright's Chromium).
Exit code 0 = every check passed.
"""
import importlib.util
import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
RUNNER = ROOT / "free" / "tests" / "ui" / "run_selftest_ci.py"


def main():
    args = sys.argv[1:]
    exe = args[args.index("--exe") + 1] if "--exe" in args else os.environ.get("LEXO_SERVER_EXE", "lexo-server")
    proc = subprocess.Popen([exe, "--selftest"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        url = proc.stdout.readline().strip()
        if not url.startswith("http://127.0.0.1:"):
            print("The server did not print its address:", url, proc.stderr.read() if proc.poll() is not None else "")
            return 1
        print("Rust server:", url, flush=True)
        spec = importlib.util.spec_from_file_location("run_selftest_ci", RUNNER)
        runner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runner)

        class Fake:                                   # what serve_selftest.start_synthetic gives: (server, app, url, cleanup)
            @staticmethod
            def start_synthetic():
                return None, None, url, lambda: None
        runner.load_serve_module = lambda: Fake
        return runner.main()
    finally:
        proc.terminate()
        try:
            proc.wait(10)
        except subprocess.TimeoutExpired:
            proc.kill()
        time.sleep(0.1)


if __name__ == "__main__":
    sys.exit(main())
