"""App is assembled from parts by area (app_*.py); no part may grow back into one huge file."""
import unittest
from pathlib import Path

from helpers import ROOT  # noqa: F401  (puts the package on sys.path)
from lexo.app import App

LEXO = Path(__file__).resolve().parent.parent / "lexo"
MAX_LINES = 450


class AppPartsTests(unittest.TestCase):
    def test_no_part_is_too_big(self):
        big = {p.name: len(p.read_text(encoding="utf-8").splitlines()) for p in LEXO.glob("app*.py")}
        self.assertEqual({k: v for k, v in big.items() if v > MAX_LINES}, {}, "split the part by area")

    def test_the_methods_the_server_calls_are_there(self):
        for name in ("open_project", "refresh", "entries", "entry_save", "glossary_payload", "proof_set", "snapshot_create",
                     "fix_apply", "replace_apply", "mt_batch", "publish_plan", "lint_start", "overview", "validation_list"):
            self.assertTrue(callable(getattr(App, name, None)), name)

    def test_parts_do_not_define_the_same_method_twice(self):
        seen = {}
        for cls in App.__mro__[1:-1]:
            for name, value in vars(cls).items():
                if callable(value) or isinstance(value, (staticmethod, classmethod)):
                    self.assertNotIn(name, seen, f"{name} is defined in {seen.get(name)} and {cls.__name__}")
                    seen[name] = cls.__name__


if __name__ == "__main__":
    unittest.main()
