"""Golden data: Core still gives exactly the recorded answers to the fixed inputs (core/golden), which is what a port is checked against."""
import json
import re
import unittest

import golden_tool as gt

REGENERATE = "if the change is meant: py core/tests/golden_tool.py, and look at the diff of core/golden/expected/ before committing it"


class GoldenTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.got = gt.collect()

    def expected(self, rel):
        return json.loads((gt.EXPECTED / rel).read_text(encoding="utf-8"))

    def check(self, rel):
        got = json.loads(json.dumps(self.got[rel], sort_keys=True, ensure_ascii=False))
        want = self.expected(rel)
        if got != want:
            for case in sorted(set(got) | set(want)) if isinstance(got, dict) else []:
                self.assertEqual(got.get(case), want.get(case), f"{rel}: {case} ({REGENERATE})")
            self.assertEqual(got, want, f"{rel} ({REGENERATE})")

    def test_the_parser_gives_the_recorded_entries_for_every_case(self):
        self.check("rpy.json")

    def test_the_archive_reader_gives_the_recorded_index_and_refuses_the_bad_ones(self):
        self.check("rpa.json")
        errors = {k: v["error"] for k, v in self.got["rpa.json"].items() if "error" in v}
        self.assertEqual(sorted(errors), ["garbage", "hostile", "truncated"])

    def test_the_layout_proposals(self):
        self.check("layout.json")

    def test_the_menu_finder(self):
        self.check("menus.json")

    def test_the_validator(self):
        self.check("validator.json")

    def test_the_spelling_hints(self):
        self.check("spelling.json")

    def test_find_and_replace_in_the_plain_modes(self):
        self.check("textedit.json")

    def test_the_font_metrics_reader(self):
        self.check("fontmetrics.json")
        self.assertEqual([k for k, v in self.got["fontmetrics.json"].items() if v is None], ["bad_upm.ttf", "no_hmtx.ttf", "not_a_font.bin", "truncated.ttf"])

    def test_every_data_scenario_gives_the_recorded_answers_and_files(self):
        for name in gt.data_names():
            with self.subTest(data=name):
                self.check(f"data/{name}.json")

    def test_every_scenario_gives_the_recorded_answers_and_files(self):
        for name in gt.scenario_names():
            with self.subTest(scenario=name):
                self.check(f"scenarios/{name}.json")


class InputsTests(unittest.TestCase):
    def test_every_input_has_an_expected_answer_and_no_answer_is_stale(self):
        have = {p.relative_to(gt.EXPECTED).as_posix() for p in gt.EXPECTED.rglob("*.json")}
        want = {"rpy.json", "rpa.json", "layout.json", "menus.json", "validator.json", "spelling.json", "textedit.json", "fontmetrics.json"} | {f"scenarios/{n}.json" for n in gt.scenario_names()} | {f"data/{n}.json" for n in gt.data_names()}
        self.assertEqual(have, want, REGENERATE)

    def test_every_parser_case_file_is_listed_and_every_listed_file_exists(self):
        listed = {spec.get("file", name + ".rpy") for name, spec in gt.load_json(gt.GOLDEN / "rpy" / "cases.json").items() if not name.startswith("_")}
        files = {p.name for p in (gt.GOLDEN / "rpy" / "cases").glob("*.rpy")}
        self.assertEqual(files, listed)

    def test_every_scenario_names_a_project_and_only_calls_core_operations(self):
        from lexo import api
        for name in gt.scenario_names():
            spec = gt.load_json(gt.GOLDEN / "scenarios" / f"{name}.json")
            self.assertTrue((gt.GOLDEN / "projects" / spec["project"]).is_dir(), name)
            for st in spec["steps"]:
                self.assertIn(st["op"], api.OPS, f"{name}: {st['op']}")

    def test_the_recorded_answers_hold_nothing_that_belongs_to_one_machine(self):
        for p in gt.EXPECTED.rglob("*.json"):
            text = p.read_text(encoding="utf-8")
            self.assertIsNone(re.search(r"[A-Za-z]:[\\/]", text.replace("\\\\", "\\")) if "<TMP>" not in text else re.search(r"[A-Za-z]:\\\\Users", text), p.name)
            self.assertNotIn("filip", text.lower(), p.name)

    def test_the_inputs_are_not_touched_by_a_checkout(self):
        """The digests in the answers are those of the bytes in the inputs, so line ends must not change: .gitattributes marks the folder as not text."""
        attrs = (gt.ROOT / ".gitattributes").read_text(encoding="utf-8")
        self.assertIn("core/golden/** -text", attrs)


if __name__ == "__main__":
    unittest.main()
