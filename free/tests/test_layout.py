"""Project layout file (project.json): groups and sections are defined per project, not in the code."""
import json
import unittest

from helpers import TempCase, dialog

from lexo import layout


class ParseTests(unittest.TestCase):
    def test_valid_layout(self):
        lay = layout.parse({"labels": {"group": "Chapter"},
                            "sections": [{"n": 1, "groups": [1, 2]}, {"n": 2, "status": "planned"}]})
        self.assertEqual(lay.active_groups, (1, 2))
        self.assertEqual(lay.section_of(2), 1)
        self.assertIsNone(lay.section_of(3))
        self.assertEqual(lay.labels, {"section": "Season", "group": "Chapter"})

    def test_round_trip(self):
        lay = layout.Layout(layout.BADIK_SECTIONS)
        again = layout.parse(lay.to_json())
        self.assertEqual(again.sections, lay.sections)

    def test_bad_layouts_are_rejected_with_a_reason(self):
        for raw, word in [([], "object"), ({"sections": [{"groups": [1]}]}, "integer"),
                          ({"sections": [{"n": 1, "groups": [1]}, {"n": 1}]}, "twice"),
                          ({"sections": [{"n": 1, "groups": [1]}, {"n": 2, "groups": [1]}]}, "more than one"),
                          ({"sections": [{"n": 1, "groups": [0]}]}, "positive"),
                          ({"sections": [{"n": 1, "status": "x"}]}, "status")]:
            with self.subTest(raw=raw), self.assertRaises(layout.LayoutError) as cm:
                layout.parse(raw)
            self.assertIn(word, str(cm.exception))


class LoadTests(TempCase):
    def data_dir(self):
        d = self.root / "_progress_data"
        d.mkdir(exist_ok=True)
        return d

    def test_no_file_means_no_groups(self):
        self.use_layout(None)
        lay, note = layout.load(self.data_dir())
        self.assertEqual((lay.active_groups, note), ((), ""))

    def test_legacy_data_without_a_file_keeps_the_first_games_layout(self):
        self.use_layout(None)
        (self.data_dir() / "prirazeni.csv").write_text("soubor;zarazeni\n", encoding="utf-8")
        lay, _ = layout.load(self.data_dir())
        self.assertEqual(lay.active_groups, tuple(range(1, 9)))

    def test_broken_file_is_reported_and_ignored(self):
        (self.data_dir() / layout.FILE_NAME).write_text("{not json", encoding="utf-8")
        lay, note = layout.load(self.data_dir())
        self.assertEqual(lay.active_groups, ())
        self.assertIn("ignored", note)


class AppTests(TempCase):
    def project(self):
        self.write("a.rpy", dialog("a_1", "e", "Hello there", "Ahoj"))
        self.write("b.rpy", dialog("b_1", "e", "Second line", "Druhý řádek"))

    def test_custom_layout_drives_groups_and_the_overview(self):
        self.use_layout([{"n": 1, "status": "active", "episodes": (10, 11)}], {"group": "Chapter"})
        self.project()
        app = self.new_app()
        self.assertEqual(layout.ACTIVE_EPISODES, (10, 11))
        self.assertEqual([e["n"] for e in app.overview()["episodes"]], [10, 11])

    def test_project_without_groups_opens_and_reports_everything_as_unassigned(self):
        self.use_layout(None)
        self.project()
        app = self.new_app()
        ov = app.overview()
        self.assertEqual(ov["episodes"], [])
        self.assertEqual(ov["seasons"], [])
        self.assertEqual(ov["unassigned"]["files"], 2)

    def test_switching_projects_switches_the_layout(self):
        self.project()
        app = self.new_app()
        self.assertEqual(layout.ACTIVE_EPISODES, tuple(range(1, 9)))
        other = self.tmp / "other"
        other.mkdir()
        (other / "a.rpy").write_text(dialog("a_1", "e", "Hi", "Ahoj"), encoding="utf-8")
        app.open_project(str(other))
        self.assertEqual(layout.ACTIVE_EPISODES, ())


if __name__ == "__main__":
    unittest.main()
