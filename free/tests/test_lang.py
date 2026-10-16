"""Language packs: language-dependent checks follow the `language` of the project."""
import unittest

from helpers import TempCase, dialog

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
        self.assertIs(lang.get("CS"), lang.CS)
        self.assertIsNone(lang.get("xx"))
        self.assertIsNone(lang.get(""))


class ProjectLanguageTests(TempCase):
    def test_language_comes_from_the_project_file(self):
        self.use_layout(None, language="")
        self.assertIs(lang.current(), lang.NEUTRAL)
        self.use_layout(layout.BADIK_SECTIONS, language="cs")
        self.assertIs(lang.current(), lang.CS)

    def test_open_project_applies_the_language_and_reports_an_unknown_one(self):
        self.write("a.rpy", dialog("a_1", "e", "Hi", "Ahoj"))
        self.use_layout([], language="cs")
        app = self.new_app()
        self.assertIs(lang.current(), lang.CS)
        self.use_layout([], language="xx")
        app = self.new_app()
        self.assertIs(lang.current(), lang.NEUTRAL)
        self.assertTrue(any("no language pack" in n for n in app.notices))

    def test_legacy_data_keeps_czech(self):
        self.use_layout(None, language="")
        d = self.root / "_progress_data"
        d.mkdir(exist_ok=True)
        (d / "epizody.csv").write_text("epizoda;stav\n", encoding="utf-8")
        lay, _ = layout.load(d)
        self.assertEqual(lay.language, "cs")


if __name__ == "__main__":
    unittest.main()
