"""Messages of the Python side: English templates that travel to the browser as templates and are translated there."""
import copy
import json
import pickle
import unittest

from helpers import ROOT  # noqa: F401  (puts the package on sys.path)

from lexo.app import AppError
from lexo.messages import Message, msg, msgn, render, text_of, wire


class MessageTests(unittest.TestCase):
    def test_a_message_is_its_english_text(self):
        m = msg("Cannot save {name}: {reason}", name="a.rpy", reason="locked")
        self.assertEqual(m, "Cannot save a.rpy: locked")
        self.assertEqual((m.template, m.params), ("Cannot save {name}: {reason}", {"name": "a.rpy", "reason": "locked"}))
        self.assertIsInstance(m, str)

    def test_braces_that_are_not_placeholders_survive(self):
        self.assertEqual(msg("The tag {tag} is open.", tag="{/b}"), "The tag {/b} is open.")
        self.assertEqual(msg("a {unknown} b"), "a {unknown} b")

    def test_counted_messages_pick_the_english_form(self):
        self.assertEqual(msgn(1, "{n} line", "{n} lines"), "1 line")
        self.assertEqual(msgn(5, "{n} line", "{n} lines"), "5 lines")
        self.assertEqual(wire(msgn(3, "{n} line", "{n} lines"))["$n"], "{n} lines")

    def test_wire_replaces_messages_everywhere_and_render_reverses_it(self):
        payload = {"ok": False, "error": msg("Bad {x}", x=1), "list": [msg("A"), "plain", 3],
                   "nested": msg("Outer {inner}", inner=msg("Inner {y}", y=2)), "n": msgn(2, "{n} file", "{n} files")}
        w = wire(payload)
        json.dumps(w)                                             # JSON-ready
        self.assertEqual(w["error"], {"$m": "Bad {x}", "p": {"x": 1}})
        self.assertEqual(w["nested"]["p"]["inner"], {"$m": "Inner {y}", "p": {"y": 2}})
        self.assertEqual(render(w), {"ok": False, "error": "Bad 1", "list": ["A", "plain", 3], "nested": "Outer Inner 2", "n": "2 files"})

    def test_text_of_keeps_the_template_of_an_exception(self):
        m = msg("Unknown file: {name}", name="x")
        self.assertIs(text_of(AppError(m)), m)
        self.assertEqual(text_of(AppError("plain")), "plain")
        self.assertEqual(text_of(ValueError()), "")

    def test_copies_and_pickles_keep_the_template(self):
        m = msg("Hello {who}", who="you")
        for c in (copy.copy(m), copy.deepcopy(m), pickle.loads(pickle.dumps(m))):
            self.assertEqual((c, c.template, c.params), ("Hello you", "Hello {who}", {"who": "you"}))


if __name__ == "__main__":
    unittest.main()
