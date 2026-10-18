"""The event log: changes are written per author, merged from other authors, and reconciled with the CSV files."""
import json
import unittest

from helpers import TempCase
from lexo import events
from lexo.messages import render

LATER = "2999-01-01T00:00:00.000000+00:00"
EARLIER = "2000-01-01T00:00:00.000000+00:00"


def event(kind, key, value, t=LATER, author="anna-1111"):
    return json.dumps({"v": 1, "t": t, "a": author, "kind": kind, "key": key, "value": value}) + "\n"


class EventLogTests(TempCase):
    def setUp(self):
        super().setUp()
        self.basic_project()
        self.events_dir = self.root / ".aside" / "events"

    def lines(self, app):
        path = app.store.events.path
        return [json.loads(x) for x in path.read_text(encoding="utf-8").splitlines()] if path.exists() else []

    def other(self, *evs, name="anna-1111.jsonl"):
        self.events_dir.mkdir(parents=True, exist_ok=True)
        with open(self.events_dir / name, "a", encoding="utf-8") as f:
            f.write("".join(evs))

    def test_a_change_is_written_as_an_event_of_this_author(self):
        app = self.new_app()
        before = len(self.lines(app))
        app.store.change("t", ["assign"], lambda: app.store.assign.__setitem__("header.rpy", "gen"))
        new = self.lines(app)[before:]
        self.assertEqual([(e["kind"], e["key"], e["value"]) for e in new], [("assign", ["header.rpy"], "gen")])
        self.assertEqual(new[0]["a"], app.config.get("author"))
        self.assertEqual(app.store.events.path.parent.resolve(), self.events_dir.resolve())

    def test_the_first_open_writes_the_existing_state_as_events(self):
        d = self.root / ".aside"
        d.mkdir(exist_ok=True)
        (d / "assignments.csv").write_text("file;group\nscript.rpy;episode 1\n", encoding="utf-8")
        app = self.new_app()
        got = {(e["kind"], tuple(e["key"])): e["value"] for e in self.lines(app)}
        self.assertEqual(got[("assign", ("script.rpy",))], "ep:1")

    def test_events_of_other_authors_are_merged_at_open_and_written_to_the_csv(self):
        app = self.new_app()
        self.other(event("assign", ["script.rpy"], "ep:3"))
        again = self.new_app()
        self.assertEqual(again.store.assign["script.rpy"], "ep:3")
        self.assertIn("episode 3", (self.root / ".aside" / "assignments.csv").read_text(encoding="utf-8-sig"))
        self.assertTrue(any("changes by other authors" in render(n) or "change by another author" in render(n) for n in again.notices))
        self.assertEqual(self.new_app().store.assign["script.rpy"], "ep:3")                 # nothing new the third time

    def test_the_newest_event_wins_whoever_wrote_it(self):
        app = self.new_app()
        app.store.change("t", ["assign"], lambda: app.store.assign.__setitem__("script.rpy", "ep:1"))
        self.other(event("assign", ["script.rpy"], "ep:4", t=EARLIER))
        self.assertEqual(self.new_app().store.assign["script.rpy"], "ep:1")
        self.other(event("assign", ["script.rpy"], "ep:2", t=LATER))
        self.assertEqual(self.new_app().store.assign["script.rpy"], "ep:2")

    def test_undo_is_a_new_event_and_a_removal_is_replayed(self):
        app = self.new_app()
        app.store.change("t", ["assign"], lambda: app.store.assign.__setitem__("header.rpy", "gen"))
        n = len(self.lines(app))
        app.store.undo()
        last = self.lines(app)[-1]
        self.assertEqual((len(self.lines(app)), last["key"], last["value"]), (n + 1, ["header.rpy"], None))
        self.assertNotIn("header.rpy", self.new_app().store.assign)

    def test_a_removal_by_another_author_removes_it_here(self):
        app = self.new_app()
        app.store.change("t", ["assign"], lambda: app.store.assign.__setitem__("header.rpy", "gen"))
        self.other(event("assign", ["header.rpy"], None))
        self.assertNotIn("header.rpy", self.new_app().store.assign)

    def test_an_edit_of_the_csv_by_hand_becomes_events(self):
        app = self.new_app()
        csv = self.root / ".aside" / "assignments.csv"
        csv.write_text("file;group\nscript.rpy;episode 2\n", encoding="utf-8")
        app2 = self.new_app()
        self.assertEqual(app2.store.assign["script.rpy"], "ep:2")
        self.assertIn(("assign", ["script.rpy"], "ep:2"), [(e["kind"], e["key"], e["value"]) for e in self.lines(app2)])

    def test_when_the_local_note_is_lost_the_log_wins_over_old_csv_files(self):
        app = self.new_app()
        app.store.change("t", ["assign"], lambda: app.store.assign.__setitem__("header.rpy", "gen"))
        self.other(event("assign", ["header.rpy"], None))
        (app.local_dir / events.SEEN_FILE).unlink()
        again = self.new_app()
        self.assertNotIn("header.rpy", again.store.assign)

    def test_refresh_picks_up_what_others_wrote_meanwhile(self):
        app = self.new_app()
        self.other(event("assign", ["script.rpy"], "ep:2"))
        r = app.refresh(force=True)
        self.assertEqual(r["merged_events"], 1)
        self.assertEqual(app.store.assign["script.rpy"], "ep:2")

    def test_damaged_and_unfinished_lines_are_skipped(self):
        self.other("not json\n", event("assign", ["script.rpy"], "ep:2"))
        with open(self.events_dir / "anna-1111.jsonl", "ab") as f:
            f.write(event("assign", ["header.rpy"], "gen").encode("utf-8")[:-1])          # cut short, no line end
        app = self.new_app()
        self.assertEqual(app.store.assign.get("script.rpy"), "ep:2")
        self.assertNotIn("header.rpy", app.store.assign)

    def test_blank_rows_for_new_files_are_not_events(self):
        app = self.new_app()
        self.assertEqual([e for e in self.lines(app) if e["kind"] == "corr" and not (e["value"] or {}).get("c")], [])

    def test_author_names_are_safe_file_names(self):
        self.assertEqual(events.clean_author("Filip Ježíšek!"), "filip-je-ek")
        self.assertEqual(events.clean_author(""), "author")
        self.assertEqual(events.clean_author("../x"), "x")


if __name__ == "__main__":
    unittest.main()
