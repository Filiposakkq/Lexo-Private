"""Configuration, editor launching, publishing (against throw-away local git repos) and the HTTP API."""
import http.client
import json
import os
import subprocess
import sys
import unittest
from pathlib import Path
from unittest import mock

from helpers import ROOT, TempCase, dialog, rel  # noqa: F401  (ROOT = Core)

from lexo import editor
from lexo_free import cli, server
from lexo.app import AppError
from lexo.messages import render
from lexo.config import Config

FREE = Path(__file__).resolve().parent.parent            # free/ (ROOT from helpers is core/)


class ConfigTests(TempCase):
    def test_persists_and_restores_independent_of_cwd(self):
        c = Config().load()
        c.set("project_dir", str(self.root))
        c.update({"editor": {"preferred": "notepadpp", "custom": ""}})
        old = os.getcwd()
        os.chdir(self.tmp)
        self.addCleanup(os.chdir, old)
        again = Config().load()
        self.assertEqual(again.get("project_dir"), str(self.root))
        self.assertEqual(again.get("editor.preferred"), "notepadpp")
        self.assertEqual(Path(again.path), self.cfg_dir / "config.json")

    def test_corrupt_file_falls_back_to_defaults(self):
        self.cfg_dir.mkdir()
        (self.cfg_dir / "config.json").write_text("{not json", encoding="utf-8")
        c = Config().load()
        self.assertIn("cannot be loaded", c.load_error)
        self.assertEqual(c.get("editor.preferred"), "auto")

    def test_unknown_keys_kept_and_wrong_types_ignored(self):
        self.cfg_dir.mkdir()
        (self.cfg_dir / "config.json").write_text(json.dumps(
            {"future": {"x": 1}, "editor": {"preferred": 5}, "validation": {"length": {"max_ratio": 3}}}), encoding="utf-8")
        c = Config().load()
        self.assertEqual(c.get("future.x"), 1)
        self.assertEqual(c.get("editor.preferred"), "auto")
        self.assertEqual(c.get("validation.length.max_ratio"), 3)
        self.assertEqual(c.get("validation.length.min_ratio"), 0.45)

    def test_project_dir_is_remembered_across_launches(self):
        self.basic_project()
        app = self.new_app()
        self.assertEqual(Config().load().get("project_dir"), str(self.root.resolve()))
        path, note = cli.choose_project_dir(Config().load(), None)
        self.assertEqual((path, note), (str(self.root.resolve()), ""))

    def test_saved_dir_that_disappeared_is_reported(self):
        c = Config().load()
        c.set("project_dir", str(self.tmp / "gone"))
        with mock.patch("lexo_free.cli.app_dir", return_value=self.tmp / "nowhere"):
            path, note = cli.choose_project_dir(c, None)
        self.assertIsNone(path)
        self.assertIn("does not exist", note)

    def test_explicit_folder_wins_and_bad_explicit_is_an_error(self):
        self.basic_project()
        c = Config().load()
        c.set("project_dir", str(self.tmp / "other"))
        self.assertEqual(cli.choose_project_dir(c, str(self.root))[0], str(self.root))
        path, note = cli.choose_project_dir(c, str(self.tmp / "nope"))
        self.assertIsNone(path)
        self.assertTrue(note)

    def test_open_project_rejects_invalid_folder(self):
        app = self.new_app(open_root=False)
        with self.assertRaises(AppError):
            app.open_project(self.tmp / "nope")
        with self.assertRaises(AppError):
            app.open_project(self.root)      # exists but has no .rpy files
        self.assertFalse(app.ready)


class WebSettingsTests(TempCase):

    def test_app_works_without_web_settings(self):
        self.basic_project()
        app = self.new_app()
        self.assertEqual(app.config.get("web.repo"), "")
        self.assertTrue(app.overview()["totals"]["total"])
        with self.assertRaises(AppError) as cm:
            app.publish_plan()                      # publishing is a Pro feature: without the pack it is locked (the setup message is tested in reference/python/pro/tests)
        self.assertIn("Pro feature", str(cm.exception))


class EditorTests(TempCase):
    def setUp(self):
        super().setUp()
        self.f = self.write("we ird & name (1).rpy", dialog("a", "mc", "Hi", "Ahoj"))
        self.calls = []
        self.runner = lambda argv: self.calls.append(argv)

    def finder(self, available):
        return lambda kind: available.get(kind)

    def test_vscode_goes_to_exact_line_with_one_argument(self):
        r = editor.open_in_editor(self.f, 42, {"preferred": "vscode"}, root=self.root, runner=self.runner,
                                  finder=self.finder({"vscode": "C:/VS/Code.exe"}))
        self.assertTrue(r["ok"])
        self.assertEqual(self.calls, [["C:/VS/Code.exe", "-g", f"{self.f.resolve()}:42"]])
        self.assertIn("&", self.calls[0][2])         # special characters stay inside one argv element
        self.assertTrue(r["jumped"])

    def test_notepadpp_command(self):
        editor.open_in_editor(self.f, 7, {"preferred": "notepadpp"}, root=self.root, runner=self.runner,
                              finder=self.finder({"notepadpp": "C:/N++/notepad++.exe"}))
        self.assertEqual(self.calls, [["C:/N++/notepad++.exe", "-n7", str(self.f.resolve())]])

    def test_falls_back_to_other_editor_then_system(self):
        r = editor.open_in_editor(self.f, 3, {"preferred": "vscode"}, root=self.root, runner=self.runner,
                                  finder=self.finder({"notepadpp": "C:/N++/notepad++.exe"}))
        self.assertTrue(r["ok"] and r["editor"] == "notepadpp")
        opened = []
        r = editor.open_in_editor(self.f, 3, {"preferred": "auto"}, root=self.root, runner=self.runner,
                                  finder=self.finder({}), opener=opened.append)
        self.assertTrue(r["ok"])
        self.assertFalse(r["jumped"])
        self.assertEqual(r["editor"], "system")
        self.assertIn("without jumping to line", r["message"])
        self.assertEqual(len(opened), 1)

    def test_failure_is_reported_not_silent(self):
        def no(_p):
            raise OSError("no association")
        r = editor.open_in_editor(self.f, 3, {"preferred": "auto"}, root=self.root, runner=self.runner,
                                  finder=self.finder({}), opener=no)
        self.assertFalse(r["ok"])
        self.assertIn("could not be opened", r["message"])

    def test_runner_error_tries_next_editor(self):
        def runner(argv):
            if "Code.exe" in argv[0]:
                raise OSError("boom")
            self.calls.append(argv)
        r = editor.open_in_editor(self.f, 3, {"preferred": "vscode"}, root=self.root, runner=runner,
                                  finder=self.finder({"vscode": "C:/VS/Code.exe", "notepadpp": "C:/N/notepad++.exe"}))
        self.assertEqual(r["editor"], "notepadpp")

    def test_refuses_files_outside_project_or_not_rpy(self):
        other = self.write("outside.rpy", "x", base=self.tmp)
        r = editor.open_in_editor(other, 1, {}, root=self.root, runner=self.runner, finder=self.finder({"vscode": "x"}))
        self.assertFalse(r["ok"])
        txt = self.write("notes.txt", "x")
        self.assertFalse(editor.open_in_editor(txt, 1, {}, root=self.root, runner=self.runner, finder=self.finder({"vscode": "x"}))["ok"])
        self.assertFalse(editor.open_in_editor(self.root / "missing.rpy", 1, {}, root=self.root, runner=self.runner)["ok"])
        self.assertEqual(self.calls, [])

    def test_never_uses_a_shell(self):
        with mock.patch("subprocess.Popen") as popen:
            editor.open_in_editor(self.f, 1, {"preferred": "vscode"}, root=self.root, finder=self.finder({"vscode": "C:/VS/Code.exe"}))
        args, kwargs = popen.call_args
        self.assertIsInstance(args[0], list)
        self.assertFalse(kwargs.get("shell"))

    def test_cmd_launcher_with_dangerous_path_is_refused(self):
        bad = self.write("a&calc.rpy", dialog("a", "mc", "Hi", "Ahoj"))
        r = editor.open_in_editor(bad, 1, {"preferred": "vscode"}, root=self.root, runner=self.runner,
                                  finder=self.finder({"vscode": "C:/VS/bin/code.cmd"}), opener=lambda p: None)
        self.assertEqual(self.calls, [])
        self.assertEqual(r["editor"], "system")

    def test_custom_command_template(self):
        exe = self.write("ed.exe", "")
        r = editor.open_in_editor(self.f, 12, {"preferred": "custom", "custom": f'"{exe}" --line {{line}} {{file}}'},
                                  root=self.root, runner=self.runner, finder=self.finder({}))
        self.assertTrue(r["ok"])
        self.assertEqual(self.calls[0], [str(exe), "--line", "12", str(self.f.resolve())])

    def test_default_finder_returns_a_path_or_none_never_a_function(self):
        for kind in ("vscode", "notepadpp"):
            found = editor.default_finder(kind)
            self.assertTrue(found is None or isinstance(found, str), found)

    def test_real_default_lookup_works_end_to_end(self):
        """Regression: without an injected finder the editor path used to be a function object."""
        fake = self.tmp / "Code.exe"
        fake.write_text("")
        with mock.patch.object(editor, "find_vscode", return_value=str(fake)):
            r = editor.open_in_editor(self.f, 5, {"preferred": "vscode"}, root=self.root, runner=self.runner)
        self.assertTrue(r["ok"], r)
        self.assertEqual(self.calls[0][:2], [str(fake), "-g"])

    def test_line_is_clamped(self):
        editor.open_in_editor(self.f, -5, {"preferred": "vscode"}, root=self.root, runner=self.runner, finder=self.finder({"vscode": "C:/V/Code.exe"}))
        self.assertTrue(self.calls[0][2].endswith(":1"))

    def test_find_vscode_prefers_exe_next_to_cmd_launcher(self):
        (self.tmp / "vs" / "bin").mkdir(parents=True)
        (self.tmp / "vs" / "bin" / "code.cmd").write_text("")
        (self.tmp / "vs" / "Code.exe").write_text("")
        found = editor.find_vscode(which=lambda n: str(self.tmp / "vs" / "bin" / "code.cmd") if n == "code" else None, env={})
        # compare resolved paths: CI temp folders use 8.3 short names (RUNNER~1) that resolve() expands
        self.assertEqual(Path(found).resolve(), (self.tmp / "vs" / "Code.exe").resolve())


class PublishCodeTests(unittest.TestCase):
    def test_no_auto_publish_code_path_remains(self):
        src = (FREE / "lexo_free" / "cli.py").read_text(encoding="utf-8")
        self.assertNotIn("push: true", src)
        self.assertNotIn('["push"]', src)


class ServerTests(TempCase):
    def setUp(self):
        super().setUp()
        self.basic_project()
        self.app = self.new_app()
        self.srv, self.sess, self.url = server.start_server(self.app, pick_folder=lambda: str(self.root))
        self.addCleanup(self.srv.shutdown)
        self.port = self.srv.server_address[1]

    def call(self, method, path, body=None, token=True, host=None):
        c = http.client.HTTPConnection("127.0.0.1", self.port, timeout=10)
        headers = {}
        if host:
            headers["Host"] = host
        if method == "POST":
            if token:
                headers["X-Token"] = self.sess.token
            headers["Content-Type"] = "application/json"
        c.request(method, path, json.dumps(body) if body is not None else None, headers)
        r = c.getresponse()
        raw = r.read()
        c.close()
        try:
            return r.status, json.loads(raw), r
        except ValueError:
            return r.status, raw, r

    def test_page_and_headers(self):
        st, body, r = self.call("GET", "/")
        self.assertEqual(st, 200)
        self.assertIn(self.sess.token.encode(), body)
        self.assertIn("default-src 'self'", r.getheader("Content-Security-Policy"))
        self.assertEqual(self.call("GET", "/../../etc/passwd")[0], 404)
        self.assertEqual(self.call("GET", "/styles.css")[0], 200)

    def test_post_needs_token_and_host_must_be_loopback(self):
        self.assertEqual(self.call("POST", "/api/undo", {}, token=False)[0], 403)
        self.assertEqual(self.call("GET", "/api/overview", host="evil.example.com")[0], 403)
        self.assertEqual(self.call("GET", "/api/overview", host="localhost")[0], 200)

    def test_read_endpoints(self):
        st, ov, _ = self.call("GET", "/api/overview")
        self.assertTrue(ov["ok"])
        self.assertEqual([s["n"] for s in ov["seasons"]], [1, 2, 3])
        _, files, _ = self.call("GET", "/api/files")
        self.assertEqual(len(files["files"]), 4)
        _, ents, _ = self.call("GET", "/api/entries?status=todo&limit=2")
        self.assertEqual(len(ents["items"]), 2)
        self.assertGreater(ents["total"], 2)
        self.assertTrue(all(i["status"] == "todo" for i in ents["items"]))
        it = ents["items"][0]
        _, det, _ = self.call("GET", f"/api/entry?file={it['file']}&line={it['line']}")
        self.assertEqual(det["line"], it["line"])
        self.assertIn(it["file"], det["path"])

    def test_filters_paging_and_around(self):
        _, all_, _ = self.call("GET", "/api/entries?status=all")
        self.assertEqual(all_["total"], 3 + 2 + 2 + 2)
        _, q, _ = self.call("GET", "/api/entries?status=all&q=party")
        self.assertEqual({i["file"] for i in q["items"]}, {rel("scripts", "episode2", "ep2_party.rpy")})
        _, page2, _ = self.call("GET", "/api/entries?status=all&limit=3&offset=3")
        self.assertEqual(len(page2["items"]), 3)
        _, around, _ = self.call("GET", f"/api/entries?file=script.rpy&status=all&around={all_['items'][1]['line']}")
        self.assertEqual(around["offset"], 0)

    def test_edit_assign_approve_undo_roundtrip(self):
        st, r, _ = self.call("POST", "/api/assign", {"name": "script.rpy", "v": "ep:1"})
        self.assertTrue(r["ok"])
        st, r, _ = self.call("POST", "/api/assign", {"name": "script.rpy", "v": "ep:9"})
        self.assertEqual(st, 400)
        _, ov, _ = self.call("GET", "/api/overview")
        self.assertEqual(ov["episodes"][0]["files"], 1)
        _, todo, _ = self.call("GET", "/api/entries?status=todo&file=script.rpy")
        it = todo["items"][0]
        _, det, _ = self.call("GET", f"/api/entry?file=script.rpy&line={it['line']}")
        done0 = ov["totals"]["done"]
        self.call("POST", "/api/approve/apply", {"selections": [{"file": "script.rpy", "key": det["key"]}], "on": True})
        _, ov2, _ = self.call("GET", "/api/overview")
        self.assertEqual(ov2["totals"]["done"], done0 + 1)
        st, u, _ = self.call("POST", "/api/undo", {})
        self.assertTrue(u["ok"])
        _, ov3, _ = self.call("GET", "/api/overview")
        self.assertEqual(ov3["totals"]["done"], done0)
        self.assertEqual(render(ov3["undo"][0]), "Assignment of script.rpy")

    def test_counts_edit_via_api_matches_old_semantics(self):
        self.call("POST", "/api/file/save", {"name": "script.rpy", "c": "2", "t": "3"})
        _, files, _ = self.call("GET", "/api/files")
        f = next(x for x in files["files"] if x["name"] == "script.rpy")
        self.assertEqual((f["done"], f["corr"], f["auto"]), (3, 2, 1))
        self.call("POST", "/api/file/save", {"name": "script.rpy", "c": "2", "t": "1"})   # equals automatic -> no override
        self.assertEqual(self.app.store.corr["script.rpy"]["t"], "")

    def test_bulk_assign_over_api(self):
        items = [{"name": rel("scripts", "episode2", "ep2_party.rpy"), "value": "ep:2"}, {"name": rel("season2", "scripts", "update5.rpy"), "value": "ep:5"}]
        _, res, _ = self.call("POST", "/api/assign/apply", {"items": items})              # the suggestions are a Pro feature; applying them is not
        self.assertEqual(len(res["applied"]), len(items))
        _, ov, _ = self.call("GET", "/api/overview")
        self.assertGreater(ov["seasons"][1]["translation"]["total"], 0)

    def test_refresh_protects_unsaved_edits_and_reports_changes(self):
        self.write("added.rpy", dialog("n", "mc", "Hello", "Ahoj"))
        st, r, _ = self.call("POST", "/api/refresh", {"dirty": True})
        self.assertTrue(r["needs_confirm"])
        self.assertNotIn("added.rpy", self.app.project.files)
        st, r, _ = self.call("POST", "/api/refresh", {"dirty": True, "discard": True})
        self.assertEqual(r["added"], ["added.rpy"])
        st, r, _ = self.call("POST", "/api/refresh", {})
        self.assertFalse(r["changed"])

    def test_refresh_retries_failed_saves_and_warns(self):
        self.app.store.unsaved.add("assign")
        with mock.patch.object(type(self.app.store), "save", side_effect=__import__("lexo.util", fromlist=["x"]).WriteFailed("x", "locked")):
            st, r, _ = self.call("POST", "/api/refresh", {})
        self.assertTrue(r["needs_confirm"])

    def test_project_switching_and_invalid_paths(self):
        st, r, _ = self.call("POST", "/api/project/set", {"path": str(self.tmp / "nope")})
        self.assertEqual(st, 400)
        self.assertIn("does not exist", render(r["error"]))
        self.assertEqual(self.call("POST", "/api/project/set", {"path": str(self.root)})[0], 200)
        st, r, _ = self.call("POST", "/api/project/pick", {})
        self.assertEqual(r["path"], str(self.root))

    def test_validation_endpoint(self):
        self.write("bad.rpy", 'translate czech x:\n\n    # mc "Hi [name]"\n    mc "Ahoj [jmeno]"\n')
        self.call("POST", "/api/refresh", {})
        _, v, _ = self.call("GET", "/api/validation?severity=error")
        self.assertEqual(v["total"], 1)
        self.assertEqual(v["items"][0]["code"], "variable-unknown")
        _, ov, _ = self.call("GET", "/api/overview")
        self.assertEqual(ov["validation"]["error"], 1)

    def test_settings_validation(self):
        st, r, _ = self.call("POST", "/api/settings", {"editor": {"preferred": "emacs"}})
        self.assertEqual(st, 400)
        st, r, _ = self.call("POST", "/api/settings", {"validation": {"length": {"max_ratio": 99}}})
        self.assertEqual(st, 400)
        st, r, _ = self.call("POST", "/api/settings", {"validation": {"length": {"max_ratio": 3.5}}})
        self.assertEqual(r["validation"]["length"]["max_ratio"], 3.5)

    def test_ui_prefs_persist_on_server(self):
        self.call("POST", "/api/prefs", {"files": {"colw": {"name": 555}}})
        self.assertEqual(Config().load().get("ui.files.colw.name"), 555)
        _, st, _ = self.call("GET", "/api/state")
        self.assertEqual(st["prefs"]["files"]["colw"]["name"], 555)

    def test_many_parallel_requests_are_all_served(self):
        """Regression: a small listen backlog reset ~1 in 15 large downloads when a browser fired many at once."""
        from concurrent.futures import ThreadPoolExecutor

        def fetch(_):
            c = http.client.HTTPConnection("127.0.0.1", self.port, timeout=20)
            c.request("GET", "/js/22-entries.js")
            r = c.getresponse()
            body = r.read()
            return r.status == 200 and len(body) > 20000
        with ThreadPoolExecutor(max_workers=48) as pool:
            results = list(pool.map(fetch, range(400)))
        self.assertTrue(all(results), f"{results.count(False)} of {len(results)} requests failed")

    def test_malformed_requests(self):
        c = http.client.HTTPConnection("127.0.0.1", self.port)
        c.request("POST", "/api/assign", b"{broken", {"X-Token": self.sess.token})
        self.assertEqual(c.getresponse().status, 400)
        self.assertEqual(self.call("POST", "/api/assign", {})[0], 400)       # missing field
        self.assertEqual(self.call("POST", "/api/nonexistent", {})[0], 404)

    def test_open_editor_endpoint_reports_result(self):
        with mock.patch("lexo.editor.open_in_editor", return_value={"ok": False, "message": "nope"}) as m:
            _, r, _ = self.call("POST", "/api/open-editor", {"file": "script.rpy", "line": 10})
        self.assertFalse(r["ok"])
        self.assertEqual(m.call_args.args[1], 10)


class CliTests(TempCase):
    def run_cli(self, *args, cwd=None):
        env = dict(os.environ, PYTHONIOENCODING="utf-8", LEXO_CONFIG_DIR=str(self.cfg_dir))
        return subprocess.run([sys.executable, str(FREE / "run_lexo.py"), *args], cwd=cwd or self.tmp,
                              capture_output=True, text=True, env=env, timeout=60, encoding="utf-8")

    def test_console_summary_writes_data_next_to_project_not_cwd(self):
        self.basic_project()
        elsewhere = self.tmp / "elsewhere"
        elsewhere.mkdir()
        r = self.run_cli(str(self.root), "--nogui", cwd=elsewhere)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("TRANSLATION", r.stdout)
        self.assertTrue((self.local_dir() / "progress.json").exists())
        self.assertTrue((self.local_dir() / "progress.csv").exists())
        self.assertEqual(list(elsewhere.iterdir()), [])
        self.assertEqual(json.loads((self.cfg_dir / "config.json").read_text(encoding="utf-8"))["project_dir"], str(self.root.resolve()))

    def test_publish_flag_never_pushes_without_typed_confirmation(self):
        self.basic_project()
        r = self.run_cli(str(self.root), "--nogui", "--publish")
        self.assertNotEqual(r.returncode, 0)
        self.assertNotIn("Publikováno", r.stdout)

    def test_bad_folder_gives_clear_error(self):
        r = self.run_cli(str(self.tmp / "nothing"), "--nogui")
        self.assertEqual(r.returncode, 2)
        self.assertIn("does not exist", r.stdout)


if __name__ == "__main__":
    unittest.main()
