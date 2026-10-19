#!/usr/bin/env python3
"""Lexo – launcher.

Run `py run_lexo.py` for the window; command line options are listed in README.md.
"""
import os
import sys
from pathlib import Path

if not getattr(sys, "frozen", False):
    here = Path(__file__).resolve().parent
    sys.path[:0] = [str(here), str(here.parent / "core")]      # Free (lexo_free) and Core (lexo)
    if (here.parent / "pro" / "lexo_pro").is_dir() and not os.environ.get("LEXO_NO_PRO"):
        sys.path.append(str(here.parent / "pro"))              # the Pro pack of a development checkout (a licensed installation has it installed)

from lexo_free.cli import run  # noqa: E402

if __name__ == "__main__":
    run()
