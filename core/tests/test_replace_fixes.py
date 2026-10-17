"""Find & replace in translations, the safe text fixes, and the batch-edit core they share."""
import unittest

from helpers import TempCase, dialog

from lexo import textedit
from lexo.app import AppError


class ReplacerTests(unittest.TestCase):
    def test_plain_word_and_case(self):
        f = textedit.make_replacer("kočka", "pes", "plain", case=False)
        self.assertEqual(f("Kočka a kočkaté"), ("pes a pesté", 2))
        f = textedit.make_replacer("kočka", "pes", "word", case=True)
        self.assertEqual(f("kočka, Kočka, kočkaté"), ("pes, Kočka, kočkaté", 1))

    def test_tags_variables_and_escapes_are_protected(self):
        f = textedit.make_replacer("n", "X", "plain")
        self.assertEqual(f("a{nw}n[name]\\nn"), ("a{nw}X[name]\\nX", 2))
        self.assertEqual(f("a{nw}n[name]\\nn", protect=False)[1], 5)

    def test_stem_mode_keeps_the_ending_of_other_forms(self):
        f = textedit.make_replacer("dikmince", "dikmunce", "stem")
        self.assertEqual(f("Vezmi dikmince, dikminci a Dikmincemi."), ("Vezmi dikmunce, dikmunci a Dikmuncemi.", 3))

    def test_bad_input(self):
        with self.assertRaises(ValueError):
            textedit.make_replacer("", "x")
        with self.assertRaises(ValueError):
            textedit.make_replacer("ab", "x", "stem")
        with self.assertRaises(ValueError):                                   # regular expressions are a Pro feature: the app asks the pack, Core never makes one
            textedit.make_replacer("a.*b", "X", "regex")


class ReplaceInAppTests(TempCase):
    def setUp(self):
        super().setUp()
        self.p1 = self.write("a.rpy", dialog("a1", "mc", "Take the Dik Coins.", "Vezmi si dikmince.", 5)
                             + dialog("a2", "mc", "More Dik Coins!", "Další dikmince {i}ihned{/i}!", 9)
                             + dialog("a3", "mc", "Nothing.", None, 14))
        self.p2 = self.write("b.rpy", dialog("b1", "de", "Coins.", "Dikmince.", 5))
        self.app = self.new_app()

    def test_preview_lists_only_translated_lines_and_writes_nothing(self):
        before = self.p1.read_bytes()
        pv = self.app.replace_preview("dikmince", "mince", "word")
        self.assertEqual((pv["total"], pv["matches"], pv["applicable"]), (3, 3, 3))
        self.assertEqual(self.p1.read_bytes(), before)
        self.assertEqual(self.app.replace_preview("nothing", "x")["total"], 0)     # untranslated lines are skipped

    def test_apply_replaces_in_all_chosen_files_as_one_undo_step(self):
        pv = self.app.replace_preview("dikmince", "mince", "word")
        sel = [{"file": i["file"], "line": i["line"]} for i in pv["items"]]
        res = self.app.replace_apply(sel, "dikmince", "mince", "word")
        self.assertEqual((res["changed"], res["files"]), (3, 2))
        self.assertIn('"Vezmi si mince."', self.p1.read_text(encoding="utf-8"))
        self.assertIn('"Další mince {i}ihned{/i}!"', self.p1.read_text(encoding="utf-8"))
        self.assertIn('"mince."', self.p2.read_text(encoding="utf-8"))
        self.app.fix_undo()
        self.assertIn("dikmince", self.p1.read_text(encoding="utf-8"))
        self.assertIn("Dikmince", self.p2.read_text(encoding="utf-8"))
        self.assertIsNone(self.app.fix_last()["last"])

    def test_only_ticked_lines_and_chosen_files_change(self):
        pv = self.app.replace_preview("dikmince", "mince", "word", files=["a.rpy"])
        self.assertEqual({i["file"] for i in pv["items"]}, {"a.rpy"})
        first = pv["items"][0]
        self.app.replace_apply([{"file": first["file"], "line": first["line"]}], "dikmince", "mince", "word", files=["a.rpy"])
        self.assertEqual(self.p1.read_text(encoding="utf-8").count("dikmince"), 1)
        self.assertIn("Dikmince", self.p2.read_text(encoding="utf-8"))

    def test_a_replacement_that_would_break_the_string_is_skipped(self):
        pv = self.app.replace_preview("Vezmi", 'Řekl "ahoj"', "plain")
        self.assertEqual((pv["total"], pv["applicable"]), (1, 0))
        self.assertIn("quote", pv["items"][0]["skip"].lower())
        with self.assertRaises(AppError):
            self.app.replace_apply([{"file": "a.rpy", "line": pv["items"][0]["line"]}], "Vezmi", 'Řekl "ahoj"', "plain")

    def test_errors_are_reported_to_the_user(self):
        with self.assertRaises(AppError):
            self.app.replace_preview("a", "b", "plain", files=["nope.rpy"])
        with self.assertRaises(AppError):
            self.app.replace_apply([], "a", "b")

    def test_refuses_when_a_file_changed_on_disk_and_leaves_every_file_alone(self):
        pv = self.app.replace_preview("dikmince", "mince", "word")
        sel = [{"file": i["file"], "line": i["line"]} for i in pv["items"]]
        edited = self.p2.read_text(encoding="utf-8") + "\n# changed elsewhere\n"
        self.p2.write_text(edited, encoding="utf-8")
        a_before = self.p1.read_bytes()
        with self.assertRaises(AppError):
            self.app.replace_apply(sel, "dikmince", "mince", "word")
        self.assertEqual(self.p1.read_bytes(), a_before)           # staging fails before anything is written
        self.assertEqual(self.p2.read_text(encoding="utf-8"), edited)


if __name__ == "__main__":
    unittest.main()
