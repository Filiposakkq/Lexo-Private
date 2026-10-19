"""Regular expression find and replace (the Pro pack, replace.regex): the substitution, the protection of tags, and the call through the app."""
import unittest

from helpers import TempCase, dialog

import lexo_pro
from lexo.app import AppError, FeatureLocked
from lexo.extensions import ExtensionError, ExtensionHost
from lexo_pro import regex_replace


def sub(pattern, replace, *texts, case=False, protect=True):
    return regex_replace.substitute({"pattern": pattern, "replace": replace, "case": case, "protect": protect, "texts": list(texts)})["results"]


class SubstituteTests(unittest.TestCase):
    def test_groups_case_and_the_count(self):
        r = sub(r"(\w+)ovi", r"\1u", "Petrovi a Honzovi", "nothing")
        self.assertEqual([(x["new"], x["n"]) for x in r], [("Petru a Honzu", 2), ("nothing", 0)])
        self.assertEqual(sub("hello", "x", "Hello")[0]["n"], 1)                    # case does not matter by default
        self.assertEqual(sub("hello", "x", "Hello", case=True)[0]["n"], 0)

    def test_it_cannot_match_across_a_tag_or_inside_one(self):
        self.assertEqual(sub(r"a.*b", "X", "a{i}b")[0], {"new": "a{i}b", "n": 0})
        self.assertEqual(sub("i", "X", "a{i}[name]b i")[0]["new"], "a{i}[name]b X")
        self.assertEqual(sub("i", "X", "{i}", protect=False)[0]["n"], 1)

    def test_bad_input_is_a_message_for_the_user(self):
        for pattern, replace in (("(", "x"), ("a", r"\9"), ("", "x")):
            with self.assertRaises(ExtensionError, msg=pattern):
                sub(pattern, replace, "text")


class RegexInTheAppTests(TempCase):
    def setUp(self):
        super().setUp()
        self.write("a.rpy", dialog("a1", "mc", "Take the Dik Coins.", "Vezmi si dikmince.", 5) + dialog("a2", "mc", "More.", "Dalsi dikmince {i}ihned{/i}!", 9))
        self.app = self.new_app()
        self.app.ext = ExtensionHost()
        self.app.ext.register(lexo_pro.provider())

    def test_preview_and_apply_go_through_the_pack(self):
        pv = self.app.replace_preview(r"dik(\w+)", r"DIK-\1", "regex")
        self.assertEqual((pv["total"], pv["applicable"]), (2, 2))
        self.assertIn("DIK-mince", pv["items"][0]["after"])
        res = self.app.replace_apply([{"file": "a.rpy", "line": pv["items"][0]["line"]}], r"dik(\w+)", r"DIK-\1", "regex")
        self.assertEqual(res["changed"], 1)

    def test_a_bad_pattern_is_an_error_message(self):
        with self.assertRaises(AppError):
            self.app.replace_preview("(", "x", "regex")

    def test_without_the_pack_regex_is_locked_but_plain_replace_is_not(self):
        self.app.ext = ExtensionHost()
        with self.assertRaises(FeatureLocked):
            self.app.replace_preview("a.*", "b", "regex")
        self.assertEqual(self.app.replace_preview("dikmince", "X", "plain")["total"], 2)

    def test_the_demo_pack_has_no_regex(self):
        import os
        os.environ["LEXO_PRO_DEMO"] = "1"
        self.addCleanup(os.environ.pop, "LEXO_PRO_DEMO", None)
        with self.assertRaises(FeatureLocked):
            self.app.replace_preview("a.*", "b", "regex")


if __name__ == "__main__":
    unittest.main()
