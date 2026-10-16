"""Interface texts: every t('English text') of the browser UI has a Czech text in the pack, nothing stale is left in the pack, and
the modules that are already converted contain no Czech string literals any more (the list only grows)."""
import json
import re
import unittest
from pathlib import Path

from helpers import ROOT  # noqa: F401  (puts the package on sys.path)

from lexo import lang

UI = Path(__file__).resolve().parent.parent / "lexo" / "ui"
PACK = Path(__file__).resolve().parent.parent / "lexo" / "packs" / "cs"

# Modules whose Czech literals have all been moved to t() / tn() / tx(). Add a module here when it is done.
CONVERTED = ["00-core.js", "10-api.js", "13-feedback.js", "12-chrome.js", "14-renderers.js", "23-fixes.js", "24-machine-translation.js", "25-replace.js", "26-approve-rules.js", "15-filterbar.js", "16-tabs.js", "17-project.js", "18-refresh.js", "99-boot.js"]

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
    return {f.name: f.read_text(encoding="utf-8") for f in sorted((UI / "js").glob("*.js"))}


def used_keys():
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
    html = (UI / "index.html").read_text(encoding="utf-8")
    keys.update(re.findall(r'data-i18n(?:-title|-aria)?="([^"]*)"', html))
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
        stale = sorted(k for k in self.strings if k not in used_keys())
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
            code = strip_comments(src[name])
            # an English text may contain Czech example words (byt / být); it is fine when it is a key of the pack
            bad = [m.group(0)[:60] for m in LITERAL.finditer(code)
                   if CZECH.search(next(g for g in m.groups() if g is not None))
                   and unescape(next(g for g in m.groups() if g is not None)) not in self.strings]
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


if __name__ == "__main__":
    unittest.main()
