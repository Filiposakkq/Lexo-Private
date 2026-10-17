"""Suggesting groups for files (the Pro pack, bulk.categorize): the heuristics, and the call through the extension host with the limit of the demo mode."""
import json
import unittest

from helpers import EIGHT_EPISODES, TempCase, dialog, rel

from lexo import layout
from lexo.app import AppError
from lexo.extensions import ExtensionHost
from lexo.messages import render
from lexo_pro import categorize
import lexo_pro


class SuggestTests(unittest.TestCase):
    def setUp(self):
        layout.use(layout.Layout(EIGHT_EPISODES, language="cs"))
        self.addCleanup(layout.use, layout.Layout(language="cs"))

    def one(self, name, current=None, known=None):
        cur = current or {}
        return categorize.suggest([name], cur, known_assign=known if known is not None else cur, lay=layout.current())[0]

    def test_episode_in_file_or_folder_name(self):
        for name, ep in [(rel("scripts", "freeroam", "episode3", "ep3_x.rpy"), "ep:3"),
                         (rel("season2", "scripts", "madame_ep7.rpy"), "ep:7"),
                         (rel("scripts", "episode4", "bella.rpy"), "ep:4")]:
            s = self.one(name)
            self.assertEqual((s["suggested"], s["state"]), (ep, "new"), name)

    def test_no_false_match_inside_words(self):
        self.assertEqual(self.one("step5_map.rpy")["suggested"], "")
        self.assertEqual(self.one("deep2.rpy")["suggested"], "")

    def test_update_files_follow_project_convention(self):
        s = self.one(rel("season2", "scripts", "update6.rpy"))
        self.assertEqual((s["suggested"], s["confidence"]), ("ep:6", "medium"))

    def test_ambiguous_when_several_episodes(self):
        s = self.one("ep2_ep3_mix.rpy")
        self.assertEqual((s["state"], s["suggested"]), ("ambiguous", ""))
        self.assertEqual(s["candidates"], ["ep:2", "ep:3"])

    def test_season_folder_resolves_ambiguity(self):
        s = self.one(rel("season2", "ep3_ep6.rpy"))
        self.assertEqual(s["suggested"], "ep:6")

    def test_conflict_between_season_folder_and_episode(self):
        s = self.one(rel("season2", "scripts", "ep3_oops.rpy"))
        self.assertEqual(s["state"], "conflict")
        self.assertEqual(s["confidence"], "low")

    def test_unknown_episode_number_not_suggested(self):
        s = self.one("ep12_future.rpy")
        self.assertEqual((s["suggested"], s["state"]), ("", "unknown"))
        self.assertIn("1–8", s["reason"])

    def test_manual_assignment_protected(self):
        s = self.one("ep4_x.rpy", current={"ep4_x.rpy": "ep:2"})
        self.assertEqual((s["suggested"], s["state"], s["overwrite"]), ("ep:4", "conflict", True))
        same = self.one("ep4_x.rpy", current={"ep4_x.rpy": "ep:4"})
        self.assertEqual(same["state"], "same")

    def test_learns_from_siblings_and_shared_names(self):
        known = {rel("scripts", "misc", "a.rpy"): "gen"}
        s = self.one(rel("scripts", "misc", "b.rpy"), known=known)
        self.assertEqual((s["suggested"], s["confidence"]), ("gen", "medium"))
        self.assertEqual(self.one("screens.rpy")["suggested"], "gen")

    def test_season_folder_without_episode_is_explained(self):
        s = self.one(rel("season2", "scripts", "laptop", "laptop.rpy"))
        self.assertEqual(s["suggested"], "")
        self.assertIn("season 2", s["reason"])


class CategorizeInTheAppTests(TempCase):
    """The call through the app and the host: the data crosses as plain JSON, the layout travels in the request."""

    def setUp(self):
        super().setUp()
        self.use_layout(None)                                       # a project without a layout file
        for n in (1, 2, 3):
            self.write(f"ep{n}_story.rpy", dialog(f"a{n}", "mc", f"Hello {n}, friend.", None, 5))
        self.write("options.rpy", dialog("o1", "mc", "Options text here.", None, 5))
        self.app = self.new_app()
        self.app.ext = ExtensionHost()
        self.app.ext.register(lexo_pro.provider())

    def test_without_groups_only_the_whole_game_files_are_suggested(self):
        sug = self.app.assign_suggest()
        self.assertFalse(sug["groups_defined"])
        self.assertEqual(sorted(i["suggested"] for i in sug["items"]), ["", "", "", "gen"])

    def test_applying_the_groups_makes_the_suggestions_work(self):
        self.app.groups_apply("Episode", [1, 2, 3])
        sug = {i["name"]: i for i in self.app.assign_suggest()["items"]}
        self.assertEqual((sug["ep2_story.rpy"]["suggested"], sug["ep2_story.rpy"]["confidence"]), ("ep:2", "high"))
        self.assertEqual(sug["options.rpy"]["suggested"], "gen")            # the name of a file of the whole game
        self.assertEqual([o["value"] for o in self.app.assign_suggest()["options"]], ["", "gen", "ep:1", "ep:2", "ep:3"])
        json.dumps(render(self.app.assign_suggest()))                      # plain JSON, the reasons are messages

    def test_the_word_is_used_in_labels_and_in_the_suggestions(self):
        for n in (1, 2, 3):
            (self.root / f"ep{n}_story.rpy").rename(self.root / f"chapter_{n}.rpy")
        self.app.refresh(force=True)
        self.app.groups_apply("Chapter", [1, 2, 3])
        opts = {o["value"]: str(o["label"]) for o in self.app.assign_suggest()["options"]}
        self.assertEqual(opts["ep:2"], "Chapter 2")
        sug = {i["name"]: i for i in self.app.assign_suggest()["items"]}
        self.assertEqual(sug["chapter_3.rpy"]["suggested"], "ep:3")

    def test_the_series_makes_script_files_suggest_their_group(self):
        for n in (1, 2, 3):
            (self.root / f"ep{n}_story.rpy").rename(self.root / ("script.rpy" if n == 1 else f"script{n}.rpy"))
        self.app.refresh(force=True)
        s = self.app.groups_suggest()
        self.app.groups_apply("Episode", [1, 2, 3], s["proposal"]["series"])
        sug = {i["name"]: i["suggested"] for i in self.app.assign_suggest()["items"]}
        self.assertEqual((sug["script.rpy"], sug["script2.rpy"], sug["script3.rpy"], sug["options.rpy"]), ("ep:1", "ep:2", "ep:3", "gen"))

    def test_one_group_per_folder(self):
        from pathlib import Path
        for d in ("home", "beach", "work"):
            (self.root / d).mkdir()
            for n in ("a", "b"):
                self.write(f"{d}/{n}.rpy", dialog(f"{d}{n}", "mc", f"Hello from {d} {n}.", None, 5))
        self.app.refresh(force=True)
        s = self.app.groups_suggest()
        self.app.groups_apply("Episode", [], "", s["proposal"]["folders"])
        opts = {o["value"]: str(o["label"]) for o in self.app.assign_suggest()["options"]}
        self.assertEqual((opts["ep:1"], opts["ep:3"]), ("beach", "work"))
        sug = {i["name"]: i["suggested"] for i in self.app.assign_suggest()["items"]}
        self.assertEqual((sug[str(Path("home") / "a.rpy")], sug[str(Path("work") / "b.rpy")]), ("ep:2", "ep:3"))


class DemoModeTests(TempCase):
    def setUp(self):
        super().setUp()
        self.use_layout(None)
        for n in range(1, 26):
            self.write(f"ep{n}_story.rpy", dialog(f"a{n}", "mc", f"Hello {n}, friend.", None, 5))
        self.app = self.new_app()
        self.app.groups_apply("Episode", list(range(1, 26)))
        self.app.ext = ExtensionHost()
        self.app.ext.register(lexo_pro.provider())

    def test_the_licensed_pack_suggests_for_every_file(self):
        r = self.app.assign_suggest()
        self.assertEqual((len(r["items"]), "limited" in r), (25, False))

    def test_the_demo_pack_cuts_the_list_and_says_so(self):
        import os
        os.environ["LEXO_PRO_DEMO"] = "1"
        self.addCleanup(os.environ.pop, "LEXO_PRO_DEMO", None)
        self.assertEqual(self.app.ext.decide("bulk.categorize").limit, 20)
        self.assertFalse(self.app.ext.decide("publish.web").available)        # publishing has no demo
        r = self.app.assign_suggest()
        self.assertEqual(len(r["items"]), 20)
        self.assertEqual(r["limited"], {"feature": "bulk.categorize", "limit": 20, "total": 25})

    def test_without_the_pack_the_suggestions_are_locked(self):
        from lexo.app import FeatureLocked
        self.app.ext = ExtensionHost()
        with self.assertRaises(FeatureLocked):
            self.app.assign_suggest()


if __name__ == "__main__":
    unittest.main()
