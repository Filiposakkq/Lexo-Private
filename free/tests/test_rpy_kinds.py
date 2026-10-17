"""Kinds of pending lines are codes (the UI words them), not Czech sentences."""
import unittest
from pathlib import Path

from helpers import TempCase, dialog
from dataclasses import replace

from lexo import lang, rpy


class KindTests(TempCase):
    def test_classify_returns_codes(self):
        self.assertEqual(rpy.classify(""), "empty")
        self.assertEqual(rpy.classify("..."), "symbols")
        self.assertEqual(rpy.classify("I am going to the park today"), "english")
        self.assertEqual(rpy.classify("Ahoj"), "same")

    def test_the_api_sends_the_code_and_no_czech_label(self):
        self.write("a.rpy", dialog("a1", "mc", "Where are you going to be today", None, 5))
        app = self.new_app()
        item = app.entries(file="a.rpy", status="all")["items"][0]
        self.assertEqual(item["kind"], "english")
        detail = app.entry_detail("a.rpy", item["line"])
        self.assertEqual(detail["kind"], "english")
        self.assertNotIn("kind_label", detail)


class KeywordSpeakerTests(unittest.TestCase):
    def test_a_character_called_old_or_new_is_a_speaker_in_a_dialogue_block(self):
        text = ("translate czech block_1:\n\n"
                '    # old "Who are you?!" with hpunch\n    old "Who are you?!" with hpunch\n\n'
                "translate czech strings:\n\n"
                '    old "Hello"\n    new "Ahoj"\n')
        parsed = rpy.parse_text(text)
        self.assertEqual(parsed.structure, [])
        self.assertEqual([(e.unit, e.who, e.source) for e in parsed.entries], [("dialogue", "old", "Who are you?!"), ("string", "", "Hello")])


class LettersOfTheLanguageTests(unittest.TestCase):
    """rpy.classify tells a translated line from an English one by the letters of the target language (language pack), not by Czech letters."""

    def setUp(self):
        self.addCleanup(lang.use, lang.CS)

    def test_czech_letters_come_from_the_czech_pack(self):
        lang.use(lang.CS)
        self.assertIn("č", lang.CS.letters)
        self.assertEqual(rpy.classify("Příliš žluťoučký kůň"), "same")

    def test_another_language_uses_its_own_letters(self):
        german = replace(lang.NEUTRAL, code="de", letters=frozenset("äöüß"))
        lang.use(german)
        self.assertEqual(rpy.classify("schöne Grüße an alle meine Freunde hier"), "same")           # has ö and ü
        lang.use(replace(lang.NEUTRAL, code="fr", letters=frozenset("éèêà")))
        self.assertEqual(rpy.classify("schöne Grüße an alle meine Freunde hier"), "english")        # none of its letters, five words

    def test_without_letters_any_non_ascii_letter_counts(self):
        lang.use(lang.NEUTRAL)
        self.assertEqual(rpy.classify("Hello there my friend how are you"), "english")
        self.assertEqual(rpy.classify("Привет всем моим друзьям"), "same")
        self.assertEqual(rpy.classify("Hallo schöne Welt"), "same")

    def test_a_pack_must_give_letters_as_text(self):
        import json
        import shutil
        import tempfile
        tmp = Path(tempfile.mkdtemp(prefix="lexo-pack-"))
        self.addCleanup(shutil.rmtree, tmp, ignore_errors=True)
        (tmp / "pack.json").write_text(json.dumps({"format": 1, "code": "xx", "name": "Test"}), encoding="utf-8")
        (tmp / "rules.json").write_text(json.dumps({"letters": ["ä"]}), encoding="utf-8")
        with self.assertRaises(lang.PackError):
            lang.load_dir(tmp)
        (tmp / "rules.json").write_text(json.dumps({"letters": "äö"}), encoding="utf-8")
        self.assertEqual(lang.load_dir(tmp).letters, frozenset("äö"))


if __name__ == "__main__":
    unittest.main()
