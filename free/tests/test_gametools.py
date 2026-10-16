"""Game tools: .rpa extraction, copying scripts, decompiling through an external helper, generating translation files."""
import os
import pickle
import shutil
import tempfile
import unittest
import zlib
from pathlib import Path

from helpers import ROOT  # noqa: F401  (puts the package on sys.path)

from lexo import gametools, rpa


def make_rpa(path, files, key=0x42, prefix_len=0, py2=False):
    """Write a small RPA-3.0 archive. files = {name: bytes}. The first `prefix_len` bytes of every file go into the index prefix.
    py2=True writes the index like Python 2 / old Ren'Py did (pickle protocol 2, str prefix); otherwise like Ren'Py 8 (protocol 4, bytes)."""
    body, index = b"", {}
    header_len = 34
    for name, data in files.items():
        prefix, rest = data[:prefix_len], data[prefix_len:]
        index[name] = [((header_len + len(body)) ^ key, len(data) ^ key, prefix.decode("latin-1") if py2 else prefix)]
        body += rest
    idx = zlib.compress(pickle.dumps(index, protocol=2 if py2 else 4))
    head = f"RPA-3.0 {header_len + len(body):016x} {key:08x}\n".encode()
    assert len(head) == header_len, len(head)
    Path(path).write_bytes(head + body + idx)


class Tmp(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="lexo-tools-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)


class ExtractTests(Tmp):
    def test_extracts_files_with_folders_and_prefix(self):
        make_rpa(self.tmp / "a.rpa", {"images/bg.png": b"PNGDATA", "scripts/x.rpyc": b"abcdef"}, prefix_len=2)
        res = gametools.extract_rpa(self.tmp / "a.rpa", self.tmp / "out")
        self.assertEqual(sorted(res["written"]), ["images/bg.png", "scripts/x.rpyc"])
        self.assertEqual((self.tmp / "out" / "images" / "bg.png").read_bytes(), b"PNGDATA")
        self.assertEqual((self.tmp / "out" / "scripts" / "x.rpyc").read_bytes(), b"abcdef")
        self.assertEqual(res["bytes"], 13)
        self.assertFalse(list((self.tmp / "out").rglob("*.part")))

    def test_old_python2_style_archives_work_too(self):
        make_rpa(self.tmp / "old.rpa", {"a.txt": b"hello"}, prefix_len=2, py2=True)
        res = gametools.extract_rpa(self.tmp / "old.rpa", self.tmp / "out")
        self.assertEqual((res["written"], (self.tmp / "out" / "a.txt").read_bytes()), (["a.txt"], b"hello"))

    def test_names_that_leave_the_target_are_skipped(self):
        evil = {"../evil.txt": b"x", "/abs.txt": b"x", "C:/win.txt": b"x", "a/../../up.txt": b"x", "ok.txt": b"fine"}
        make_rpa(self.tmp / "e.rpa", evil)
        res = gametools.extract_rpa(self.tmp / "e.rpa", self.tmp / "out")
        self.assertEqual(res["written"], ["ok.txt"])
        self.assertEqual(len(res["skipped"]), 4)
        self.assertFalse((self.tmp / "evil.txt").exists())
        self.assertFalse((self.tmp / "up.txt").exists())

    def test_existing_files_are_kept_unless_overwrite(self):
        make_rpa(self.tmp / "a.rpa", {"f.txt": b"new"})
        out = self.tmp / "out"
        out.mkdir()
        (out / "f.txt").write_bytes(b"old")
        res = gametools.extract_rpa(self.tmp / "a.rpa", out)
        self.assertEqual((res["written"], (out / "f.txt").read_bytes()), ([], b"old"))
        self.assertEqual(res["skipped"][0][0], "f.txt")
        gametools.extract_rpa(self.tmp / "a.rpa", out, overwrite=True)
        self.assertEqual((out / "f.txt").read_bytes(), b"new")

    def test_filter_and_limits(self):
        make_rpa(self.tmp / "a.rpa", {"a.png": b"1234", "b.ogg": b"5678"})
        res = gametools.extract_rpa(self.tmp / "a.rpa", self.tmp / "o1", only=lambda n: n.endswith(".png"))
        self.assertEqual(res["written"], ["a.png"])
        res = gametools.extract_rpa(self.tmp / "a.rpa", self.tmp / "o2", max_file=3)
        self.assertEqual(res["written"], [])
        self.assertEqual({r for _n, r in res["skipped"]}, {"soubor je příliš velký"})
        res = gametools.extract_rpa(self.tmp / "a.rpa", self.tmp / "o3", max_total=6)
        self.assertEqual(len(res["written"]), 1)

    def test_list_find_and_bad_archives(self):
        make_rpa(self.tmp / "a.rpa", {"b.txt": b"12", "a.txt": b"123"})
        self.assertEqual(gametools.list_archive(self.tmp / "a.rpa"), [("a.txt", 3), ("b.txt", 2)])
        (self.tmp / "sub").mkdir()
        make_rpa(self.tmp / "sub" / "z.rpa", {"q": b"1"})
        self.assertEqual([p.name for p in gametools.find_archives(self.tmp)], ["a.rpa", "z.rpa"])
        (self.tmp / "bad.rpa").write_bytes(b"not an archive")
        with self.assertRaises(rpa.RpaError):
            gametools.extract_rpa(self.tmp / "bad.rpa", self.tmp / "o")
        with self.assertRaises(gametools.ToolError):
            gametools.extract_rpa(self.tmp / "a.rpa", self.tmp / "a.rpa")      # the destination is a file

    def test_safe_target(self):
        d = self.tmp / "dest"
        d.mkdir()
        self.assertEqual(gametools.safe_target(d, "a/b.txt"), (d / "a" / "b.txt").resolve())
        self.assertEqual(gametools.safe_target(d, "a\\b.txt"), (d / "a" / "b.txt").resolve())
        for bad in ("", "..", "a/../../b", "/etc/x", "D:\\x", "a\0b"):
            self.assertIsNone(gametools.safe_target(d, bad), bad)


class DecompileTests(Tmp):
    def game(self):
        g = self.tmp / "game"
        (g / "scripts").mkdir(parents=True)
        (g / "script.rpyc").write_bytes(b"1")
        (g / "scripts" / "ep1.rpyc").write_bytes(b"2")
        (g / "scripts" / "readme.txt").write_bytes(b"x")
        return g

    def test_copy_scripts_only_reads_the_game(self):
        g = self.game()
        n = gametools.copy_scripts(g, self.tmp / "work")
        self.assertEqual(n, 2)
        self.assertEqual(sorted(p.name for p in (self.tmp / "work").rglob("*") if p.is_file()), ["ep1.rpyc", "script.rpyc"])
        with self.assertRaises(gametools.ToolError):
            gametools.copy_scripts(g, g / "inside")

    def test_decompile_runs_unrpyc_as_an_external_helper(self):
        g = self.game()
        work = self.tmp / "work"
        gametools.copy_scripts(g, work)
        py, script = Path(os.sys.executable), self.tmp / "unrpyc.py"
        script.write_text("# fake", encoding="utf-8")
        seen = {}

        def runner(argv, cwd):
            seen["argv"], seen["cwd"] = argv, cwd
            (work / "script.rpy").write_text("label start:\n", encoding="utf-8")
            (work / "scripts" / "ep1.rpy").write_text("label ep1:\n", encoding="utf-8")
            return 0, "done"
        res = gametools.decompile(py, script, work, clobber=True, runner=runner)
        self.assertEqual(seen["argv"], [str(py), str(script), "-c", str(work)])
        self.assertEqual((res["ok"], res["written"]), (True, ["script.rpy", "scripts/ep1.rpy"]))
        self.assertEqual(sorted(p.name for p in g.rglob("*.rpy")), [])                   # the game folder got nothing

    def test_decompile_checks_its_inputs(self):
        work = self.tmp / "work"
        work.mkdir()
        with self.assertRaises(gametools.ToolError):
            gametools.decompile(self.tmp / "nopython.exe", self.tmp / "unrpyc.py", work)
        with self.assertRaises(gametools.ToolError):
            gametools.decompile(Path(os.sys.executable), self.tmp / "missing.py", work)


class TranslateTests(Tmp):
    def sdk(self):
        sdk = self.tmp / "sdk"
        (sdk / "lib" / "windows-x86_64").mkdir(parents=True)
        (sdk / "lib" / "windows-x86_64" / "python.exe").write_bytes(b"")
        (sdk / "renpy.py").write_text("", encoding="utf-8")
        return sdk

    def project(self):
        base = self.tmp / "proj"
        (base / "game").mkdir(parents=True)
        return base

    def test_refuses_without_confirmation_or_with_a_bad_language_or_an_existing_folder(self):
        base, sdk = self.project(), self.sdk()
        with self.assertRaises(gametools.ToolError) as cm:
            gametools.generate_translation(sdk, base, "czech", runner=lambda a, c: (0, ""))
        self.assertIn("potvrdit", str(cm.exception))
        for bad in ("", "1abc", "cz ech", "../x", "a;b"):
            with self.subTest(bad), self.assertRaises(gametools.ToolError):
                gametools.generate_translation(sdk, base, bad, write_into_project=True, runner=lambda a, c: (0, ""))
        (base / "game" / "tl" / "czech").mkdir(parents=True)
        with self.assertRaises(gametools.ToolError) as cm:
            gametools.generate_translation(sdk, base, "czech", write_into_project=True, runner=lambda a, c: (0, ""))
        self.assertIn("nebudu ji přepisovat", str(cm.exception))
        with self.assertRaises(gametools.ToolError):
            gametools.generate_translation(sdk, self.tmp / "nogame", "czech", write_into_project=True)

    def test_generates_through_the_sdk(self):
        base, sdk = self.project(), self.sdk()
        seen = {}

        def runner(argv, cwd):
            seen["argv"] = argv
            tl = base / "game" / "tl" / "czech"
            tl.mkdir(parents=True)
            (tl / "script.rpy").write_text("translate czech start_1:\n", encoding="utf-8")
            return 0, "ok"
        res = gametools.generate_translation(sdk, base, "czech", write_into_project=True, runner=runner)
        self.assertEqual(seen["argv"][-2:], ["translate", "czech"])
        self.assertEqual(seen["argv"][-3], str(base))
        self.assertEqual((res["ok"], res["files"]), (True, ["script.rpy"]))

    def test_plan_does_not_touch_anything(self):
        base = self.project()
        self.assertEqual(gametools.plan_translation(base, "czech")["exists"], False)
        self.assertFalse((base / "game" / "tl").exists())

    def test_lint_command_is_unchanged(self):
        from lexo import lint
        argv, _cwd = lint.build_command(self.sdk(), self.project())
        self.assertEqual(argv[-1], "lint")


if __name__ == "__main__":
    unittest.main()
