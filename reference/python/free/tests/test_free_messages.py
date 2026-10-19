"""The Python of Free (server, command line) holds no Czech sentences either: texts go through msg(...) with an English template."""
import unittest
from pathlib import Path

import test_python_messages as core_check            # the check of Core (reference/python/core/tests) is reused

FREE_PY = Path(__file__).resolve().parent.parent / "lexo_free"


class FreeMessagesTests(unittest.TestCase):
    def test_no_czech_sentences_are_left_in_the_python_of_free(self):
        left = [f"{f.name}: {t[:70]}" for f in sorted(FREE_PY.glob("*.py")) for t in core_check.czech_strings(f) if (f.name, t) not in core_check.DATA]
        self.assertEqual(left, [], "use msg(...) with an English template (and add the Czech text to rust/resources/packs/cs/ui.json)")


if __name__ == "__main__":
    unittest.main()
