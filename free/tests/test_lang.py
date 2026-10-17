"""Language packs: language-dependent checks follow the `language` of the project."""
import json
import unittest

from helpers import EIGHT_EPISODES, TempCase, dialog

from lexo import lang, layout, validator


def codes(src, tgt):
    return {i.code: i.severity for i in validator.check_quality(src, tgt)}


class PackTests(unittest.TestCase):
    def setUp(self):
        self.addCleanup(lang.use, lang.CS)

    def test_czech_rules(self):
        lang.use(lang.CS)
        self.assertIn("repeated-word", codes("Go go away", "Jdi pryč pryč"))
        self.assertNotIn("repeated-word", codes("So what", "Tak tak"))               # in repeat_ok
        self.assertEqual(codes("I am 14", "Je mi čtrnáct")["numbers-differ"], "info")  # number in words is normal
        self.assertTrue(validator.term_used("Viděl jsem Jaskyni", "jaskyně"))         # inflected form found by stem

    def test_neutral_rules_do_not_assume_a_language(self):
        lang.use(lang.NEUTRAL)
        self.assertNotIn("repeated-word", codes("Go go away", "Jdi pryč pryč"))
        self.assertEqual(codes("I am 14", "Je mi čtrnáct")["numbers-differ"], "warning")
        self.assertFalse(validator.term_used("Viděl jsem Jaskyni", "jaskyně"))        # exact word needed, no stemming
        self.assertTrue(validator.term_used("Viděl jsem jaskyně", "jaskyně"))

    def test_lookup(self):
        self.assertEqual(lang.get("CS").code, "cs")
        self.assertIsNone(lang.get("xx"))
        self.assertIsNone(lang.get(""))


class PackFilesTests(TempCase):
    def make(self, root, code="xx", version=1, rules=None, meta=None):
        d = root / code
        d.mkdir(parents=True, exist_ok=True)
        (d / "pack.json").write_text(json.dumps({"format": 1, "code": code, "name": code.upper(), "version": version, **(meta or {})}), encoding="utf-8")
        (d / "rules.json").write_text(json.dumps(rules if rules is not None else {}), encoding="utf-8")
        return d

    def test_the_czech_pack_is_a_data_pack(self):
        self.assertEqual((lang.CS.code, lang.CS.version), ("cs", 1))
        self.assertIn("tak", lang.CS.repeat_ok)
        self.assertEqual(lang.CS.stem("Jaskyni"), "jasky")           # drops the last 2 letters (after folding)

    def test_pack_rules_are_read_from_files(self):
        d = self.make(self.tmp / "packs", rules={"check_repeats": True, "repeat_ok": ["Ja"], "stem": {"type": "cut", "drop": 1, "min": 2}})
        p = lang.load_dir(d)
        self.assertEqual((p.code, p.check_repeats, p.repeat_ok, p.stem("Hund")), ("xx", True, frozenset({"ja"}), "hun"))

    def test_broken_packs_are_rejected_with_a_reason(self):
        for rules, meta, word in [({"stem": {"type": "magic"}}, None, "stem"), ({"repeat_ok": "x"}, None, "repeat_ok"),
                                  ({"check_repeats": 1}, None, "check_repeats"), ({}, {"format": 2}, "format"),
                                  ({}, {"version": "1"}, "version"), ({"stem": {"type": "cut", "drop": -1}}, None, "whole numbers")]:
            with self.subTest(rules=rules, meta=meta), self.assertRaises(lang.PackError) as cm:
                lang.load_dir(self.make(self.tmp / "bad", rules=rules, meta=meta))
            self.assertIn(word, str(cm.exception))

    def test_discover_prefers_a_newer_user_pack_and_skips_broken_ones(self):
        user = self.tmp / "userpacks"
        self.make(user, "cs", version=5, rules={"check_repeats": False})        # newer than the bundled cs
        self.make(user, "de", version=1)
        bad = self.make(user, "zz")
        (bad / "rules.json").write_text("{nope", encoding="utf-8")
        found = lang.discover([user])
        self.assertEqual(found["cs"].version, 5)
        self.assertIn("de", found)
        self.assertNotIn("zz", found)
        self.assertTrue(any(p.startswith("zz:") for p in lang.problems()))
        self.assertIn("und", found)

    def test_get_sees_a_pack_installed_in_the_user_folder(self):
        self.assertIsNone(lang.get("de"))
        self.make(lang.user_dir(), "de")
        self.assertEqual(lang.get("de").code, "de")


class ProjectLanguageTests(TempCase):
    def test_language_comes_from_the_project_file(self):
        self.use_layout(None, language="")
        self.assertIs(lang.current(), lang.NEUTRAL)
        self.use_layout(EIGHT_EPISODES, language="cs")
        self.assertEqual(lang.current().code, "cs")

    def test_open_project_applies_the_language_and_reports_an_unknown_one(self):
        self.write("a.rpy", dialog("a_1", "e", "Hi", "Ahoj"))
        self.use_layout([], language="cs")
        app = self.new_app()
        self.assertEqual(lang.current().code, "cs")
        self.use_layout([], language="xx")
        app = self.new_app()
        self.assertIs(lang.current(), lang.NEUTRAL)
        self.assertTrue(any("no language pack" in n for n in app.notices))


if __name__ == "__main__":
    unittest.main()
