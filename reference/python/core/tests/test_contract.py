"""The interface of Core (reference/python/core/lexo/api.py): every operation is described, answers the way it was recorded, and the generated documents are up to date."""
import json
import unittest
from pathlib import Path

import contract_tool as ct
from helpers import TempCase

from lexo import api
from lexo.app_base import AppError

# Operations without an example for the contract run, and why (a new operation must be run by the test or be named here with a reason).
NOT_EXERCISED = {
    "consistency/unify": "needs a group of originals translated differently (the fixture has none)",
    "entry/mt": "uses the network (a machine translation service)",
    "episode": "needs groups with assignments to set a state on",
    "assign/suggest": "a Pro feature (locked without the pack; tested with the pack)",
    "fix/apply": "a Pro feature (locked without the pack; tested with the pack)",
    "fix/preview": "a Pro feature (locked without the pack; tested with the pack)",
    "fix/undo": "needs a fix to undo (the fixes are a Pro feature)",
    "game/font": "binary answer; needs a font file of a game",
    "game/image": "binary answer; needs the picture archives of a game",
    "game/fontmetrics": "needs a font file of a game",
    "glossary/import-apply": "needs the characters of a game file; the preview below has no fixture either",
    "glossary/import-preview": "needs a characters file of a game",
    "lint/start": "starts the Ren'Py SDK",
    "mt/batch": "uses the network (a Pro feature)",
    "mt/targets": "a Pro feature (locked without the pack; tested with the pack)",
    "open-editor": "starts a program",
    "open-url": "opens the browser",
    "project/set": "opens another project (the fixture is the project)",
    "publish/confirm": "pushes to a git repository (a Pro feature; tested with the Pro pack)",
    "publish/plan": "a Pro feature (locked without the pack; tested with the pack)",
    "snapshots/delete": "needs the name of a snapshot",
    "snapshots/files": "needs the name of a snapshot",
    "snapshots/restore": "needs the name of a snapshot",
}


class DescriptionTests(unittest.TestCase):
    def test_every_operation_is_described(self):
        for name, op in api.OPS.items():
            self.assertTrue(op.doc and op.doc.endswith((".", ")", "}")), name)
            self.assertIn(op.method, ("GET", "POST"), name)
            self.assertIn(op.effect, ("read", "write", "external"), name)
            self.assertEqual(api.ROUTES[(op.method, op.path)], op, name)
        self.assertEqual(len(api.ROUTES), len(api.OPS))              # no two operations on one method and address

    def test_each_operation_is_run_by_the_contract_test_or_has_a_reason_here(self):
        without = {n for n, o in api.OPS.items() if o.example is None}
        self.assertEqual(without, set(NOT_EXERCISED), "a new operation needs an example in api.py, or a reason in NOT_EXERCISED")

    def test_parameters_are_coerced_the_same_way_from_a_query_string_and_from_json(self):
        entries = api.OPS["entries"]
        a = entries.bind({"limit": "7", "word": "1", "status": "todo", "offset": ""})            # a query string: everything is text
        self.assertEqual((a.limit, a.word, a.status, a.offset, a.file), (7, True, "todo", 0, ""))
        b = entries.bind({"limit": 7, "word": True})                                              # JSON: typed
        self.assertEqual((b.limit, b.word, b.status), (7, True, "all"))
        self.assertEqual(entries.bind({}).limit, 200)
        sel = api.OPS["approve/apply"].bind({})
        self.assertEqual((sel.selections, sel.on), ([], True))                                    # a flag can default to true (JSON bodies)
        self.assertFalse(api.OPS["entries"].bind({"word": "0"}).word)
        with self.assertRaises(KeyError):
            api.OPS["entry"].bind({"line": 5})                                                    # a required parameter is missing
        self.assertEqual(api.OPS["entry"].bind({"file": "a.rpy", "line": "5"}).line, 5)

    def test_an_unknown_operation_is_refused(self):
        with self.assertRaises(api.UnknownOperation):
            api.call(None, "no/such/operation")


class AnswerTests(TempCase):
    def setUp(self):
        super().setUp()
        self.app, cleanup = ct.open_fixture()          # a fresh project for every test: the examples change it
        self.addCleanup(cleanup)

    def test_the_answers_keep_the_recorded_shape(self):
        recorded = json.loads((ct.CONTRACT_DIR / "shapes.json").read_text(encoding="utf-8"))
        answers = ct.run_examples(self.app)
        self.assertEqual(set(answers), set(recorded), "run py reference/python/core/tests/contract_tool.py to record the shapes of new or removed operations")
        problems = []
        for name, answer in answers.items():
            problems += [f"{name}{p[1:]}" for p in ct.compatible(recorded[name], ct.shape(answer))]
        self.assertEqual(problems, [], "an answer lost a key or changed a type; if it is on purpose, regenerate: py reference/python/core/tests/contract_tool.py")

    def test_every_answer_is_plain_json_with_ok(self):
        for name, answer in ct.run_examples(self.app).items():
            json.dumps(answer)
            self.assertIs(answer.get("ok", True), True, name)

    def test_the_table_is_what_the_app_does(self):
        fresh, cleanup = ct.open_fixture()
        try:
            direct = fresh.entries(status="all", limit=5)
            self.assertEqual(api.call(fresh, "entries", {"status": "all", "limit": "5"})["items"], direct["items"])
            self.assertEqual(api.call(fresh, "state")["ready"], True)
        finally:
            cleanup()

    def test_an_operation_that_needs_a_project_is_refused_without_one(self):
        from lexo.app import App
        from lexo.config import Config
        app = App(Config().load())
        for name in ("overview", "entries", "entry/save"):
            with self.assertRaises(AppError, msg=name):
                api.call(app, name, {"file": "a.rpy", "line": 1})
        self.assertFalse(api.call(app, "state")["ready"])          # a few operations need none


class GeneratedFilesTests(unittest.TestCase):
    def test_the_contract_files_are_up_to_date(self):
        self.assertEqual(json.loads((ct.CONTRACT_DIR / "api.json").read_text(encoding="utf-8")), json.loads(json.dumps(api.describe())),
                         "api.py changed: regenerate with py reference/python/core/tests/contract_tool.py")

    def test_the_readable_document_is_up_to_date(self):
        doc = Path(ct.DOC)
        if not doc.exists():
            self.skipTest("docs/technical/API.md is not part of this checkout")
        self.assertEqual(doc.read_text(encoding="utf-8"), ct.render_doc(), "api.py changed: regenerate with py reference/python/core/tests/contract_tool.py")


if __name__ == "__main__":
    unittest.main()
