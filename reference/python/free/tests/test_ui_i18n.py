"""Interface texts: every t('English text') of the browser UI has a Czech text in the pack, nothing stale is left in the pack, and
the modules that are already converted contain no Czech string literals any more (the list only grows)."""
import functools
import json
import re
import unittest
from pathlib import Path

from helpers import ROOT  # noqa: F401  (puts the package on sys.path)
import jsscan

from lexo import lang

UI = Path(__file__).resolve().parents[4] / "rust" / "resources" / "ui-classic"
CORE_LEXO = Path(__file__).resolve().parents[2] / "core" / "lexo"
FREE_PY = Path(__file__).resolve().parents[1] / "lexo_free"
PACK = Path(__file__).resolve().parents[4] / "rust" / "resources" / "packs" / "cs"
# the sources of the new interface (Vue): its texts are keys of the same pack
UI_NEXT_SRC = Path(__file__).resolve().parents[4] / "ui-next-src" / "src"

# Every module of the UI is converted: no Czech string literal may remain (the English text of a key may contain Czech example words).
CONVERTED = sorted(f.name for f in (UI / "js").glob("*.js"))

CZECH = re.compile(r"[ěščřžýáíéůúťďňĚŠČŘŽÝÁÍÉŮÚŤĎŇ]")
STR = r"""(?:'((?:\\.|[^'\\\n])*)'|"((?:\\.|[^"\\\n])*)")"""
CALL_T = re.compile(r"(?<![\w$.])(?:t|T|tr|tx|N_)\(\s*" + STR)
CALL_TN = re.compile(r"(?<![\w$.])tn\(\s*[^,()]+(?:\([^()]*\))?[^,()]*,\s*" + STR + r"\s*,\s*" + STR)
LITERAL = re.compile(r"'((?:\\.|[^'\\\n])*)'|\"((?:\\.|[^\"\\\n])*)\"|`((?:\\.|[^`\\])*)`", re.S)


def unescape(s):
    return s.replace("\\'", "'").replace('\\"', '"').replace("\\\\", "\\")


def strip_comments(text):
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
    return re.sub(r"(?<![:'\"`\\])//[^\n]*", " ", text)


def js_sources():
    sources = {f.name: f.read_text(encoding="utf-8") for f in sorted((UI / "js").glob("*.js"))}
    if UI_NEXT_SRC.is_dir():       # the new interface: .vue and .js files (not the copy of the text module itself, not tests)
        for f in sorted([*UI_NEXT_SRC.rglob("*.vue"), *UI_NEXT_SRC.rglob("*.js")]):
            if f.name != "i18n.js" and not f.name.endswith(".test.js"):
                sources["next/" + f.name] = re.sub(r"<style.*?</style>", " ", f.read_text(encoding="utf-8"), flags=re.S)
    return sources


@functools.lru_cache(maxsize=None)
def _used_keys():
    keys = set()
    for name, text in js_sources().items():
        code = strip_comments(text)
        for m in CALL_T.finditer(code):
            keys.add(unescape(m.group(1) if m.group(1) is not None else m.group(2)))
        for m in CALL_TN.finditer(code):
            one = unescape(m.group(1) if m.group(1) is not None else m.group(2))
            many = unescape(m.group(3) if m.group(3) is not None else m.group(4))
            keys.add(one + "|" + many)
        for table in re.findall(r"\blabels\(\{(.*?)\}\)", code, re.S):          # labels({ key: 'English text', ... }) is translated when read
            keys.update(unescape(a or b) for a, b in re.findall(r":\s*" + STR, table))
    keys.update(python_keys())
    html = (UI / "index.html").read_text(encoding="utf-8")
    keys.update(re.findall(r'data-i18n(?:-title|-aria)?="([^"]*)"', html))
    return keys


def used_keys():
    return set(_used_keys())


def rust_keys():
    """Templates of the messages written in Rust (Core): msg("...") and msgn(n, "one", "many"). They need not be in Python, so their Czech texts are not stale."""
    keys = set()
    src = Path(__file__).resolve().parents[4] / "rust" / "crates" / "lexo-core" / "src"
    one = re.compile(r'\bmsg\(\s*"((?:[^"\\]|\\.)*)"')
    many = re.compile(r'\bmsgn\(\s*[^,()]+(?:\([^()]*\))?[^,()]*,\s*"((?:[^"\\]|\\.)*)"\s*,\s*"((?:[^"\\]|\\.)*)"')
    for f in sorted(src.glob("*.rs")):
        text = f.read_text(encoding="utf-8")
        keys.update(m.group(1).replace('\\"', '"') for m in one.finditer(text))
        keys.update(m.group(1) for m in re.finditer(r'\bfailed\(\s*"([^"]*)"', text))     # app_git.rs: the name of the step is a message of its own
        keys.update(m.group(1) for m in re.finditer(r'\brun\([^,]+,\s*"([^"]*)"\)', text))
        keys.update(m.group(1) for m in re.finditer(r'\brun\(&\[[^\]]*\],\s*"([^"]*)"\)', text))   # ... also where the step is named in a call of the runner
        keys.update(m.group(1).replace('\\"', '"') + "|" + m.group(2).replace('\\"', '"') for m in many.finditer(text))
    return keys


def python_keys():
    """Templates of the messages written in Python: msg("...") and msgn(n, "one", "many")."""
    import ast
    keys = set()
    for f in sorted([*FREE_PY.glob("*.py"), *CORE_LEXO.glob("*.py")]):                 # Free and Core
        for node in ast.walk(ast.parse(f.read_text(encoding="utf-8"))):
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id in ("msg", "msgn"):
                consts = [a.value for a in node.args if isinstance(a, ast.Constant) and isinstance(a.value, str)]
                if node.func.id == "msg" and consts:
                    keys.add(consts[0])
                elif node.func.id == "msgn" and len(consts) >= 2:
                    keys.add(consts[0] + "|" + consts[1])
    return keys


class UiTextTests(unittest.TestCase):
    def setUp(self):
        self.strings = json.loads((PACK / "ui.json").read_text(encoding="utf-8"))["strings"]

    def test_the_pack_file_is_valid(self):
        self.assertTrue(lang.load_ui_strings(lang.load_dir(PACK)))

    def test_every_used_text_has_a_czech_translation(self):
        missing = sorted(k for k in used_keys() if k not in self.strings)
        self.assertEqual(missing, [], "texts without a Czech translation in packs/cs/ui.json")

    def test_the_pack_has_no_stale_texts(self):
        known = used_keys() | rust_keys()
        stale = sorted(k for k in self.strings if k not in known)
        self.assertEqual(stale, [], "texts in packs/cs/ui.json that no t() uses any more")

    def test_placeholders_match(self):
        for key, val in self.strings.items():
            want = set(re.findall(r"\{(\w+)\}", key))
            for form in ([val] if isinstance(val, str) else val):
                got = set(re.findall(r"\{(\w+)\}", form))
                if not isinstance(val, str):          # a plural form may leave out the number itself ("one file" instead of "1 file")
                    got, want_here = got | ({"n"} & want), want
                else:
                    want_here = want
                self.assertEqual(got, want_here, f"placeholders of {key!r}")

    def test_converted_modules_have_no_czech_literals(self):
        src = js_sources()
        for name in CONVERTED:
            # an English text may contain Czech example words (byt / být); it is fine when it is a key of the pack
            bad = [t["body"][:60] for t in jsscan.scan(src[name])
                   if t["kind"] in ("str", "tpl") and CZECH.search(t["body"]) and unescape(t["body"]) not in self.strings]
            self.assertEqual(bad, [], f"{name} still has Czech literals")

    def test_no_translated_text_is_built_from_a_template_literal(self):
        for name, text in js_sources().items():
            code = strip_comments(text)
            self.assertEqual(re.findall(r"(?<![\w$.])(?:t|T|tr|tx|tn|N_)\(\s*`", code), [], f"{name}: use placeholders, not a template literal")


class PluralTests(unittest.TestCase):
    def test_python_rules(self):
        self.assertEqual([lang.plural_index("cs", n) for n in (0, 1, 2, 4, 5, 21)], [2, 0, 1, 1, 2, 2])
        self.assertEqual([lang.plural_index("en", n) for n in (0, 1, 2)], [1, 0, 1])
        self.assertEqual([lang.plural_index("pl", n) for n in (1, 2, 5, 12, 22)], [0, 1, 2, 2, 1])
        self.assertEqual([lang.plural_index("ru", n) for n in (1, 2, 5, 11, 21)], [0, 1, 2, 2, 0])

    def test_the_browser_has_the_same_rules(self):
        js = (UI / "js" / "05-i18n.js").read_text(encoding="utf-8")
        for rule in lang.PLURAL_RULES:
            self.assertRegex(js, rf"\b{rule}: \[{lang.PLURAL_RULES[rule][0]},", f"rule {rule} missing or different in 05-i18n.js")


class UiNamesTests(unittest.TestCase):
    def test_a_module_that_calls_T_imports_it(self):
        bad = []
        for path in sorted((UI / "js").glob("*.js")):
            text = path.read_text(encoding="utf-8")
            if re.search(r"(?<![\w.])T\(", text) and not re.search(r"\bt as T\b", text):
                bad.append(path.name)
        self.assertEqual(bad, [], "these modules call T() without importing it as `t as T`")


if __name__ == "__main__":
    unittest.main()
