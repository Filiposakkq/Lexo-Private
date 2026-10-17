"""Groups (episodes, chapters ...) of a project that does not come from the BaDIK layout: proposed from file names, saved, used for assignments."""
import json
import unittest
from pathlib import Path

from helpers import EIGHT_EPISODES, TempCase, dialog
from lexo import layout
from lexo.app import AppError


class ProposeTests(unittest.TestCase):
    def test_the_word_with_the_most_numbers_wins(self):
        p = layout.propose(["ep1_cafe.rpy", "scripts/ep2/a.rpy", "ep10x.rpy", "update3.rpy", "options.rpy"])
        self.assertEqual((p["label"], p["groups"], p["matched"], p["total"]), ("Episode", [1, 2, 10], 3, 5))

    def test_chapters_parts_days_updates_acts(self):
        for word, name in (("Chapter", "chapter_{n}.rpy"), ("Part", "part{n}.rpy"), ("Day", "day-{n}.rpy"), ("Update", "update_{n}.rpy"), ("Act", "act{n}.rpy")):
            self.assertEqual(layout.propose([name.format(n=n) for n in (1, 2, 3)])["label"], word, word)

    def test_nothing_recognisable(self):
        p = layout.propose(["script.rpy", "options.rpy", "ep1.rpy"])           # one number is not a series
        self.assertEqual((p["groups"], p["sections"]), ([], []))

    def test_seasons_make_sections_only_when_there_are_two(self):
        two = layout.propose(["season1/ep1.rpy", "season1/ep2.rpy", "season2/ep3.rpy", "season2/ep4.rpy"])
        self.assertEqual(two["sections"], [{"n": 1, "groups": [1, 2]}, {"n": 2, "groups": [3, 4]}])
        one = layout.propose(["season2/ep1.rpy", "season2/ep2.rpy"])
        self.assertEqual(one["sections"], [{"n": 1, "groups": [1, 2]}])

    def test_regex_follows_the_word(self):
        self.assertEqual(layout.group_regex("Chapter").findall("scripts/chapter_04/a.rpy"), ["4"])
        self.assertEqual(layout.group_regex("Chapter").findall("scripts/ep4/a.rpy"), [])

    def test_a_numbered_series_of_files_is_found_script_script2_script3(self):
        p = layout.propose(["script.rpy", "script2.rpy", "script3.rpy", "script4.rpy", "options.rpy", "screens.rpy", "gallery.rpy"])
        self.assertEqual((p["label"], p["groups"], p["series"], p["matched"]), ("Episode", [1, 2, 3, 4], "script", 4))

    def test_two_files_are_not_a_series_and_a_word_match_wins_a_tie(self):
        self.assertEqual(layout.propose(["script.rpy", "script2.rpy", "options.rpy"])["groups"], [])
        p = layout.propose(["chapter1.rpy", "chapter2.rpy", "chapter3.rpy", "script.rpy", "script2.rpy", "script3.rpy"])
        self.assertEqual((p["label"], p["series"]), ("Chapter", ""))

    def test_a_series_needs_no_gap_and_in_a_big_project_a_real_share_of_the_files(self):
        self.assertEqual(layout.propose(["bedroom1.rpy", "bedroom4.rpy", "bedroom5.rpy"])["series"], "")             # 1, 4, 5: not an episode list
        many = [f"scripts/scene{i}/a.rpy" for i in range(40)] + [f"cosplay_outfit{i}.rpy" for i in range(1, 7)]
        self.assertEqual(layout.propose(many)["series"], "")                                                          # 6 of 46 files
        small = ["script.rpy", "script2.rpy", "script3.rpy"] + [f"other{i}x.rpy" for i in range(10)]
        self.assertEqual(layout.propose(small)["series"], "script")

    def test_series_number(self):
        self.assertEqual([layout.series_number(n, "script") for n in ("script.rpy", "tl/script2.rpy", "scripts/script_03.rpy", "scripts/a.rpy", "scripts2.rpy", "")],
                         [1, 2, 3, None, None, None])
        self.assertIsNone(layout.series_number("script2.rpy", ""))

    def test_folders_are_proposed_when_the_game_is_organised_by_folder(self):
        names = [f"scripts/{d}/{n}.rpy" for d in ("home", "beach", "work", "school") for n in ("a", "b", "c")] + ["options.rpy", "screens.rpy"]
        p = layout.propose(names)
        self.assertEqual(p["folders"], ["scripts/beach", "scripts/home", "scripts/school", "scripts/work"])    # the level below "scripts"
        self.assertEqual(p["groups"], [])

    def test_no_folder_proposal_for_a_flat_game_or_a_giant_folder(self):
        self.assertEqual(layout.propose(["a.rpy", "b.rpy", "c.rpy"])["folders"], [])
        self.assertEqual(layout.propose([f"main/{i}.rpy" for i in range(20)] + ["x/a.rpy", "y/b.rpy", "z/c.rpy"])["folders"], [])    # main holds 87 %
        self.assertEqual(layout.propose(["a/1.rpy", "b/2.rpy"])["folders"], [])                                                       # two folders only

    def test_folder_number_takes_the_longest_matching_folder(self):
        folders = ["scripts/home", "scripts/home/kitchen", "bg"]
        self.assertEqual([layout.folder_number(p, folders) for p in ("scripts/home/a.rpy", "scripts/home/kitchen/b.rpy", "BG/x.rpy", "scripts/other/c.rpy", "bgx/d.rpy")],
                         [1, 2, 3, None, None])


class GroupsInTheAppTests(TempCase):
    def setUp(self):
        super().setUp()
        self.use_layout(None)                                       # a project without a layout file
        for n in (1, 2, 3):
            self.write(f"ep{n}_story.rpy", dialog(f"a{n}", "mc", f"Hello {n}, friend.", None, 5))
        self.write("options.rpy", dialog("o1", "mc", "Options text here.", None, 5))
        self.app = self.new_app()

    def test_without_groups_nothing_can_be_suggested_but_the_proposal_is_there(self):
        self.assertEqual(self.app.overview()["groups_defined"], False)
        s = self.app.groups_suggest()
        self.assertEqual((s["current"]["groups"], s["proposal"]["groups"], s["proposal"]["label"]), ([], [1, 2, 3], "Episode"))
        sug = self.app.assign_suggest()
        self.assertFalse(sug["groups_defined"])
        self.assertEqual(sorted(i["suggested"] for i in sug["items"]), ["", "", "", "gen"])

    def test_applying_the_groups_makes_the_suggestions_work(self):
        self.app.groups_apply("Episode", [1, 2, 3])
        raw = json.loads((self.app.data_dir / layout.FILE_NAME).read_text(encoding="utf-8"))
        self.assertEqual((raw["labels"]["group"], raw["sections"]), ("Episode", [{"n": 1, "status": "active", "groups": [1, 2, 3]}]))
        sug = {i["name"]: i for i in self.app.assign_suggest()["items"]}
        self.assertEqual((sug["ep2_story.rpy"]["suggested"], sug["ep2_story.rpy"]["confidence"]), ("ep:2", "high"))
        self.assertEqual(sug["options.rpy"]["suggested"], "gen")            # the name of a file of the whole game
        self.assertEqual([o["value"] for o in self.app.assign_suggest()["options"]], ["", "gen", "ep:1", "ep:2", "ep:3"])
        self.assertTrue(self.app.overview()["groups_defined"])

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
        self.assertEqual((s["proposal"]["series"], s["proposal"]["groups"]), ("script", [1, 2, 3]))
        self.app.groups_apply("Episode", [1, 2, 3], s["proposal"]["series"])
        raw = json.loads((self.app.data_dir / layout.FILE_NAME).read_text(encoding="utf-8"))
        self.assertEqual(raw["series"], "script")
        sug = {i["name"]: i["suggested"] for i in self.app.assign_suggest()["items"]}
        self.assertEqual((sug["script.rpy"], sug["script2.rpy"], sug["script3.rpy"], sug["options.rpy"]), ("ep:1", "ep:2", "ep:3", "gen"))
        again = self.new_app()                                                    # the setting survives a restart
        self.assertEqual(again.groups_suggest()["current"]["series"], "script")
        again.groups_apply("Episode", [1, 2, 3], "")
        self.assertNotIn("series", json.loads((self.app.data_dir / layout.FILE_NAME).read_text(encoding="utf-8")))

    def test_one_group_per_folder_in_the_app(self):
        for d in ("home", "beach", "work"):
            (self.root / d).mkdir()
            for n in ("a", "b"):
                self.write(f"{d}/{n}.rpy", dialog(f"{d}{n}", "mc", f"Hello from {d} {n}.", None, 5))
        self.app.refresh(force=True)
        s = self.app.groups_suggest()
        self.assertEqual(s["proposal"]["folders"], ["beach", "home", "work"])
        r = self.app.groups_apply("Episode", [], "", s["proposal"]["folders"])
        self.assertEqual((r["label"], r["groups"]), ("Folder", [1, 2, 3]))
        raw = json.loads((self.app.data_dir / layout.FILE_NAME).read_text(encoding="utf-8"))
        self.assertEqual((raw["labels"]["group"], raw["folders"]), ("Folder", ["beach", "home", "work"]))
        ov = self.app.overview()
        self.assertEqual((ov["groups_defined"], ov["group_names"]), (True, {"1": "beach", "2": "home", "3": "work"}))
        opts = {o["value"]: str(o["label"]) for o in self.app.assign_suggest()["options"]}
        self.assertEqual((opts["ep:1"], opts["ep:3"]), ("beach", "work"))
        sug = {i["name"]: i["suggested"] for i in self.app.assign_suggest()["items"]}
        self.assertEqual((sug[str(Path("home") / "a.rpy")], sug[str(Path("work") / "b.rpy")]), ("ep:2", "ep:3"))
        again = self.new_app()
        self.assertEqual(again.groups_suggest()["current"]["folders"], ["beach", "home", "work"])
        again.groups_apply("Episode", [1, 2], "", [])                                 # back to numbered groups
        self.assertNotIn("folders", json.loads((self.app.data_dir / layout.FILE_NAME).read_text(encoding="utf-8")))

    def test_it_survives_a_restart_and_keeps_other_keys(self):
        path = self.app.data_dir / layout.FILE_NAME
        path.write_text(json.dumps({"version": 1, "language": "cs", "labels": {"section": "Season"}}), encoding="utf-8")
        again = self.new_app()
        again.groups_apply("Part", [1, 2])
        raw = json.loads(path.read_text(encoding="utf-8"))
        self.assertEqual((raw["language"], raw["labels"]), ("cs", {"section": "Season", "group": "Part"}))
        self.assertEqual(self.new_app().overview()["labels"]["group"], "Part")

    def test_bad_input_is_refused(self):
        for label, groups in (("Nonsense", [1]), ("Episode", []), ("Episode", ["x"]), ("Episode", [0, 1]), ("Episode", [1000])):
            with self.assertRaises(AppError):
                self.app.groups_apply(label, groups)

    def test_several_sections_are_left_to_the_file(self):
        self.use_layout(EIGHT_EPISODES)
        app = self.new_app()
        with self.assertRaises(AppError):
            app.groups_apply("Episode", [1, 2])

    def test_http_routes(self):
        import http.client
        from lexo_free import server
        srv, sess, _ = server.start_server(self.app)
        self.addCleanup(srv.shutdown)

        def call(method, path, body=None):
            c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=10)
            c.request(method, path, json.dumps(body) if body is not None else None, {"X-Token": sess.token})
            r = c.getresponse()
            return r.status, json.loads(r.read())
        st, r = call("GET", "/api/groups/suggest")
        self.assertEqual((st, r["proposal"]["groups"]), (200, [1, 2, 3]))
        st, r = call("POST", "/api/groups/apply", {"label": "Episode", "groups": [1, 2, 3]})
        self.assertEqual((st, r["groups"]), (200, [1, 2, 3]))
        self.assertEqual(call("POST", "/api/groups/apply", {"label": "x", "groups": [1]})[0], 400)


if __name__ == "__main__":
    unittest.main()
