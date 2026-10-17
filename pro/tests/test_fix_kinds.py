"""The three kinds of safe fixes (speaker, spaces, typo): what the preview offers and what is applied (the Pro feature fixes.safe)."""
import unittest

from helpers import dialog
from procase import ProCase

from lexo.app import AppError


class SafeFixTests(ProCase):
    def setUp(self):
        super().setUp()
        body = "".join(dialog(f"a{i}", "mc", "I will come.", "Já přijdu zítra, takový je plán.", 10 + i) for i in range(6))
        body += dialog("t1", "mc", "Weird.", "Já  prijdu zítra.", 40)                      # double space + missing diacritics
        body += dialog("t2", "mc happy", "Hello.", "Ahoj.", 50).replace('    mc happy "Ahoj."', '    mc sad "Ahoj."')
        body += dialog("t3", "de", "Hello [name].", "Ahoj [jmeno].", 60)                     # variable problem: never auto-fixed
        self.path = self.write("script.rpy", body)
        self.app = self.new_app()

    def test_preview_offers_only_safe_kinds_with_sensible_defaults(self):
        pv = self.app.fix_preview(kinds=["speaker", "spaces", "typo"])
        kinds = {(i["kind"]): i for i in pv["items"]}
        self.assertEqual(set(kinds), {"speaker", "spaces", "typo"})
        self.assertTrue(kinds["spaces"]["default"])
        self.assertTrue(kinds["typo"]["default"])                                          # "prijdu" is 6 letters: sure enough
        self.assertEqual(kinds["spaces"]["after"].count("  "), 0)
        self.assertNotIn("jmeno", " ".join(i["after"] for i in pv["items"] if i["kind"] != "speaker") and "")
        self.assertFalse(any("[jmeno]" in i["note"] for i in pv["items"]))                 # variables are not offered

    def test_apply_only_what_was_ticked(self):
        pv = self.app.fix_preview(kinds=["speaker", "spaces", "typo"])
        sp = next(i for i in pv["items"] if i["kind"] == "spaces")
        res = self.app.fix_apply(selections=[{"file": sp["file"], "line": sp["line"], "kind": "spaces"}])
        self.assertEqual(res["fixed"], 1)
        text = self.path.read_text(encoding="utf-8")
        self.assertIn('"Já prijdu zítra."', text)                                          # typo left: it was not ticked
        self.assertIn("mc sad", text)                                                       # speaker left too

    def test_all_kinds_together_are_one_undo_step_per_operation(self):
        pv = self.app.fix_preview(kinds=["speaker", "spaces", "typo"])
        sel = [{"file": i["file"], "line": i["line"], "kind": i["kind"]} for i in pv["items"] if not i["skip"]]
        res = self.app.fix_apply(selections=sel)
        text = self.path.read_text(encoding="utf-8")
        self.assertIn('"Já přijdu zítra."', text)
        self.assertIn('mc happy "Ahoj."', text)
        self.assertIn('"Ahoj [jmeno]."', text)                                              # the variable line is untouched
        self.assertEqual(res["fixed"], 2)                                    # two lines: one text fix (spaces + typo), one speaker
        self.app.fix_undo()
        self.app.fix_undo()
        self.assertIn("mc sad", self.path.read_text(encoding="utf-8"))

    def test_findings_know_which_fix_applies(self):
        kinds = {}
        for it in self.app.validation_list(severity="warning")["items"] + self.app.validation_list(severity="error")["items"]:
            if it["extra"].get("fixable"):
                kinds[it["code"]] = it["extra"]["kind"]
        self.assertEqual(kinds.get("double-space"), "spaces")
        self.assertEqual(kinds.get("typo-suspect"), "typo")
        self.assertEqual(kinds.get("speaker-changed"), "speaker")
        self.assertNotIn("variable-unknown", kinds)
        self.assertNotIn("variable-missing", kinds)

    def test_nothing_ticked_is_an_error_not_a_silent_success(self):
        with self.assertRaises(AppError):
            self.app.fix_apply(selections=[])


class DemoAndLockTests(ProCase):
    def setUp(self):
        super().setUp()
        self.write("a.rpy", "".join(dialog(f"a{n}", "mc", "Hello there.", "Ahoj  tam, takhle.", 3 + n * 4) for n in range(25)))
        self.app = self.new_app()

    def test_the_licensed_pack_plans_every_fix(self):
        pv = self.app.fix_preview(kinds=["spaces"])
        self.assertEqual((len(pv["items"]), pv["total"], "limited" in pv), (25, 25, False))

    def test_the_demo_pack_cuts_the_plan_and_says_so(self):
        import os
        os.environ["LEXO_PRO_DEMO"] = "1"
        self.addCleanup(os.environ.pop, "LEXO_PRO_DEMO", None)
        pv = self.app.fix_preview(kinds=["spaces"])
        self.assertEqual((len(pv["items"]), pv["total"]), (20, 25))
        self.assertEqual(pv["limited"], {"feature": "fixes.safe", "limit": 20, "total": 25})

    def test_without_the_pack_the_fixes_are_locked(self):
        from lexo.app import FeatureLocked
        from lexo.extensions import ExtensionHost
        self.app.ext = ExtensionHost()
        with self.assertRaises(FeatureLocked):
            self.app.fix_preview(kinds=["spaces"])
        with self.assertRaises(FeatureLocked):
            self.app.fix_apply(selections=[{"file": "a.rpy", "line": 5, "kind": "spaces"}])

    def test_a_fix_is_logged_in_the_history_of_the_line_with_its_origin(self):
        pv = self.app.fix_preview(kinds=["spaces"])
        it = pv["items"][0]
        self.app.fix_apply(selections=[{"file": it["file"], "line": it["line"], "kind": "spaces"}])
        self.assertEqual(self.app.entry_history(it["file"], it["line"])["versions"][0]["via"], "fix")


if __name__ == "__main__":
    unittest.main()
