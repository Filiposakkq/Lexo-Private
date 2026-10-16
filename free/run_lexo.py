#!/usr/bin/env python3
"""Lexo – launcher.

Run `py run_lexo.py` for the window; command line options are listed in README.md.
"""
import sys
from pathlib import Path

if not getattr(sys, "frozen", False):
    sys.path.insert(0, str(Path(__file__).resolve().parent))

from lexo.cli import run  # noqa: E402

if __name__ == "__main__":
    run()
