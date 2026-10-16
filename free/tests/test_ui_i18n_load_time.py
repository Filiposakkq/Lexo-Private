"""A text translated at module load time would always be English (the language is chosen after the modules are loaded). This finds
top-level `const X = ...` statements that call t() / tn() / tx() directly, i.e. without sitting inside a function."""
import re
import unittest
from pathlib import Path

from helpers import ROOT  # noqa: F401  (puts the package on sys.path)

UI = Path(__file__).resolve().parent.parent / "lexo" / "ui"
CALL = re.compile(r"(?<![\w$.])(?:t|T|tr|tn|tx)\(\s*['\"]")
START = re.compile(r"^(?:export\s+)?(?:const|let|var)\s")


def top_level_statements(text):
    """Blocks that start at column 0 with const/let/var, up to the next line that starts at column 0 with something else."""
    lines, block, out = text.split("\n"), [], []
    for line in lines:
        if line[:1] and line[:1] not in " \t})]" and not line.startswith(("*", "/*", "//")):
            if block:
                out.append("\n".join(block))
            block = [line] if START.match(line) else []
        elif block:
            block.append(line)
    if block:
        out.append("\n".join(block))
    return out


class LoadTimeTests(unittest.TestCase):
    def test_no_text_is_translated_while_a_module_loads(self):
        bad = []
        for f in sorted((UI / "js").glob("*.js")):
            for stmt in top_level_statements(f.read_text(encoding="utf-8")):
                if "=>" in stmt or "function" in stmt or "Proxy(" in stmt:
                    continue
                if CALL.search(stmt):
                    bad.append(f"{f.name}: {stmt.splitlines()[0][:80]}")
        self.assertEqual(bad, [], "translate these when they are used (a function), or mark the texts with N_() and call t() where they are shown")


if __name__ == "__main__":
    unittest.main()
