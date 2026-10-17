#!/usr/bin/env python3
"""Lexo – launcher.

Run `py run_lexo.py` for the window; command line options are listed in README.md.
"""
import sys
from pathlib import Path

if not getattr(sys, "frozen", False):
    here = Path(__file__).resolve().parent
    sys.path[:0] = [str(here), str(here.parent / "core")]      # Free (lexo_free) and Core (lexo)

from lexo_free.cli import run  # noqa: E402

if __name__ == "__main__":
    run()
