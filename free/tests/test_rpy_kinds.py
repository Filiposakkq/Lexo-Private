"""Kinds of pending lines are codes (the UI words them), not Czech sentences."""
import unittest

from helpers import TempCase, dialog
from lexo import rpy


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


if __name__ == "__main__":
    unittest.main()
