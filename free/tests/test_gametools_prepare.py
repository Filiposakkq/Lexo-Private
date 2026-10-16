"""From a built game to translation files: repair of the decompiled scripts, files Ren'Py cannot parse, and the whole sequence."""
import shutil
import tempfile
import unittest
from pathlib import Path

from helpers import ROOT  # noqa: F401  (puts the package on sys.path)
from lexo import gametools, lint
from test_gametools import make_rpa


class RepairTests(unittest.TestCase):
    def test_lines_after_a_backslash_in_a_python_block_are_indented(self):
        text = 'init python:\n    credits = [\\\n("", "A"),\\\n("", "B")]\nlabel x:\n    "hi"\n'
        new, n = gametools.repair_continuations(text)
        self.assertEqual(n, 2)
        self.assertEqual(new, 'init python:\n    credits = [\\\n    ("", "A"),\\\n    ("", "B")]\nlabel x:\n    "hi"\n')

    def test_everything_else_is_left_alone(self):
        text = 'label start:\n    "a long line" \\\n    "second"\n\ninit python:\n    x = 1\n'
        self.assertEqual(gametools.repair_continuations(text), (text, 0))

    def test_unparsable_files_named_in_the_output_are_set_aside(self):
        tmp = Path(tempfile.mkdtemp(prefix="lexo-q-"))
        self.addCleanup(shutil.rmtree, tmp, ignore_errors=True)
        (tmp / "sub").mkdir()
        (tmp / "sub" / "bad.rpy").write_text("x", encoding="utf-8")
        (tmp / "good.rpy").write_text("y", encoding="utf-8")
        raw = 'File "game/sub/bad.rpy", line 3: expected statement.\n    foo\nFile "game/sub/bad.rpy", line 9: oops'
        self.assertEqual(gametools.quarantine_unparsable(tmp, raw), ["sub/bad.rpy"])
        self.assertTrue((tmp / "sub" / "bad.rpy.unparsable").is_file())
        self.assertFalse((tmp / "sub" / "bad.rpy").exists())
        self.assertTrue((tmp / "good.rpy").is_file())
        self.assertEqual(gametools.quarantine_unparsable(tmp, "File \"game/tl/x.rpy\", line 1: no"), [])


class PrepareTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="lexo-prep-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.game = self.tmp / "Game-1.0-pc"
        (self.game / "game").mkdir(parents=True)
        (self.game / "lib" / "py3-windows-x86_64").mkdir(parents=True)
        (self.game / "lib" / "py3-windows-x86_64" / "python.exe").write_bytes(b"py")
        (self.game / "lib" / "py3-linux-x86_64").mkdir()
        (self.game / "lib" / "py3-linux-x86_64" / "python").write_bytes(b"elf")
        (self.game / "Game.py").write_text("launcher", encoding="utf-8")
        (self.game / "Game.exe").write_bytes(b"exe")
        make_rpa(self.game / "game" / "scripts.rpa", {"a.rpyc": b"c1", "sub/b.rpyc": b"c2", "images/x.png": b"png"})
        self.unrpyc = self.tmp / "unrpyc.py"
        self.unrpyc.write_text("x", encoding="utf-8")
        self.python = self.tmp / "python.exe"
        self.python.write_bytes(b"py")

    def test_extract_scripts_only_takes_scripts(self):
        ws = self.tmp / "ws"
        gametools.make_workspace(self.game, ws)
        res = gametools.extract_scripts(ws)
        self.assertEqual((res["archives"], res["written"]), (1, 2))
        self.assertTrue((ws / "game" / "sub" / "b.rpyc").is_file())
        self.assertFalse((ws / "game" / "images").exists())

    def test_the_whole_sequence_with_stub_programs(self):
        ws, calls, steps = self.tmp / "ws", [], []

        def runner(argv, cwd):
            calls.append(argv)
            if "translate" in argv:
                tl = ws / "game" / "tl" / "czech"
                tl.mkdir(parents=True)
                (tl / "a.rpy").write_text("translate czech x:\n", encoding="utf-8")
            else:
                (ws / "game" / "a.rpy").write_text("label a:\n    return\n", encoding="utf-8")
            return 0, "ok"
        res = gametools.prepare_translation(self.game, ws, "czech", self.python, self.unrpyc, runner=runner, progress=steps.append)
        self.assertEqual(steps, ["workspace", "extract", "decompile", "generate"])
        self.assertTrue(res["ok"])
        self.assertEqual(res["generate"]["files"], ["a.rpy"])
        self.assertTrue(calls[0][1].endswith("unrpyc.py") and calls[1][-2:] == ["translate", "czech"])
        self.assertFalse((self.game / "game" / "tl").exists())             # the original is untouched
        self.assertFalse((self.game / "game" / "a.rpy").exists())

    def test_an_unparsable_script_is_set_aside_and_the_rest_is_translated(self):
        ws, runs = self.tmp / "ws", []

        def runner(argv, cwd):
            if "translate" not in argv:
                (ws / "game" / "bad.rpy").write_text("oops", encoding="utf-8")
                (ws / "game" / "a.rpy").write_text("label a:\n    return\n", encoding="utf-8")
                return 0, "ok"
            runs.append(1)
            if (ws / "game" / "bad.rpy").exists():
                (ws / "game" / "tl" / "czech").mkdir(parents=True, exist_ok=True)
                return 1, 'File "game/bad.rpy", line 1: expected statement.'
            tl = ws / "game" / "tl" / "czech"
            tl.mkdir(parents=True, exist_ok=True)
            (tl / "a.rpy").write_text("translate czech x:\n", encoding="utf-8")
            return 0, "ok"
        res = gametools.prepare_translation(self.game, ws, "czech", self.python, self.unrpyc, runner=runner)
        self.assertTrue(res["ok"])
        self.assertEqual((res["quarantined"], len(runs)), (["bad.rpy"], 2))
        self.assertTrue((ws / "game" / "bad.rpy.unparsable").is_file())

    def test_a_failing_decompile_stops_the_sequence(self):
        res = gametools.prepare_translation(self.game, self.tmp / "ws", "czech", self.python, self.unrpyc, runner=lambda a, c: (1, "boom"))
        self.assertFalse(res["ok"])
        self.assertNotIn("generate", res)

    def test_python_of_this_platform_is_chosen(self):
        py = lint.find_python(self.game)
        self.assertIn("windows" if __import__("sys").platform.startswith("win") else "linux", str(py))


if __name__ == "__main__":
    unittest.main()
