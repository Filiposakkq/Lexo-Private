"""The Python side has no Czech sentences left: every message is msg(...) / msgn(...) in English (see messages.py).
What remains Czech is DATA that is stored in files or used as an identifier (status labels in the CSV files, the kinds of lines,
the sources of the history of a line); it goes away with the new data layout (docs/DATA-LAYOUT.md)."""
import ast
import re
import unittest
from pathlib import Path

from helpers import ROOT  # noqa: F401  (puts the package on sys.path)

LEXO = Path(__file__).resolve().parent.parent / "lexo"
CZECH = re.compile(r"[ěščřžýáíéůúťďňĚŠČŘŽÝÁÍÉŮÚŤĎŇ]")

# (module, text) pairs that are data, not messages
DATA = {
    ("project.py", "Nezačato"), ("project.py", "Rozpracováno"), ("project.py", "Přeloženo"), ("project.py", "Hotovo"), ("project.py", "Korektura"),
    ("project.py", "V korekci"), ("project.py", "Plánováno"),
    ("store.py", "Nezačato"), ("store.py", "Rozpracováno"), ("store.py", "Přeloženo"), ("store.py", "Obecné"),
    ("rpy.py", "áčďéěíňóřšťúůýžÁČĎÉĚÍŇÓŘŠŤÚŮÝŽ"), ("rpy.py", "Prázdný překlad"), ("rpy.py", "Anglicky (nepřeloženo?)"),
    ("rpy.py", "Shodné – možná OK"), ("rpy.py", "Jen znaky (…, ?!)"),
    ("texthistory.py", "úprava v aplikaci"), ("texthistory.py", "nahrazení"), ("texthistory.py", "sjednocení"), ("texthistory.py", "vrácení změny"),
    ("texthistory.py", "obnovení ze zálohy"), ("texthistory.py", "změna mimo aplikaci"), ("texthistory.py", "původní text před první zaznamenanou změnou"),
}


def czech_strings(path):
    tree = ast.parse(path.read_text(encoding="utf-8"))
    docs = set()
    for n in ast.walk(tree):
        if isinstance(n, (ast.FunctionDef, ast.ClassDef, ast.Module, ast.AsyncFunctionDef)) and n.body \
                and isinstance(n.body[0], ast.Expr) and isinstance(getattr(n.body[0], "value", None), ast.Constant):
            docs.add(id(n.body[0].value))
    out = []
    for n in ast.walk(tree):
        if isinstance(n, ast.Constant) and isinstance(n.value, str) and id(n) not in docs and CZECH.search(n.value):
            out.append(n.value)
    return out


class PythonMessagesTests(unittest.TestCase):
    def test_no_czech_sentences_are_left_in_the_python_modules(self):
        left = []
        for f in sorted(LEXO.glob("*.py")):
            for text in czech_strings(f):
                if (f.name, text) not in DATA:
                    left.append(f"{f.name}: {text[:70]}")
        self.assertEqual(left, [], "use msg(...) with an English template (and add the Czech text to packs/cs/ui.json)")

    def test_messages_have_no_stray_braces(self):
        for f in sorted(LEXO.glob("*.py")):
            for node in ast.walk(ast.parse(f.read_text(encoding="utf-8"))):
                if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id in ("msg", "msgn"):
                    for a in node.args:
                        if isinstance(a, ast.Constant) and isinstance(a.value, str):
                            rest = re.sub(r"\{(\w+)\}", "", a.value)
                            self.assertNotIn("{", rest, f"{f.name}:{node.lineno} {a.value[:60]!r}")
                            self.assertNotIn("}", rest, f"{f.name}:{node.lineno} {a.value[:60]!r}")


if __name__ == "__main__":
    unittest.main()
