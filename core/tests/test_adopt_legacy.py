"""A project folder from the time when the application was called Lexo is renamed once: `.lexo` -> `.aside`, `lexo.json` -> `aside.json`."""
import tempfile
import unittest
from pathlib import Path

from lexo import datadir


class AdoptLegacyTests(unittest.TestCase):
    def test_the_old_folder_and_layout_file_are_renamed_once(self):
        with tempfile.TemporaryDirectory() as t:
            root = Path(t)
            (root / ".lexo").mkdir()
            (root / ".lexo" / "lexo.json").write_text("{}", encoding="utf-8")
            (root / ".lexo" / "project-id").write_text("abc12345\n", encoding="utf-8")
            datadir.adopt_legacy(root)
            self.assertFalse((root / ".lexo").exists())
            self.assertTrue((root / ".aside" / "aside.json").is_file())
            self.assertEqual((root / ".aside" / "project-id").read_text(encoding="utf-8"), "abc12345\n")
            datadir.adopt_legacy(root)                                   # nothing more to do
            self.assertTrue((root / ".aside" / "aside.json").is_file())

    def test_the_old_folder_is_left_alone_when_the_new_one_exists(self):
        with tempfile.TemporaryDirectory() as t:
            root = Path(t)
            (root / ".lexo").mkdir()
            (root / ".aside").mkdir()
            datadir.adopt_legacy(root)
            self.assertTrue((root / ".lexo").is_dir())


if __name__ == "__main__":
    unittest.main()
