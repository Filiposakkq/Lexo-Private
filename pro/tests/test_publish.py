"""Publishing progress to the website repository (the Pro pack), against throw-away local git repos."""
import json
import os
import unittest
from pathlib import Path

from helpers import TempCase

import lexo_pro
from lexo import export
from lexo.app import AppError
from lexo.extensions import API, ExtensionHost
from lexo.messages import render
from lexo_pro import publish as pub
import subprocess


def git(cwd, *args):
    r = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    if r.returncode != 0:
        raise AssertionError(f"git {' '.join(args)} failed: {r.stderr}")
    return r.stdout.strip()


class PublishTests(TempCase):
    def setUp(self):
        super().setUp()
        self.remote = self.tmp / "origin.git"
        self.work = self.tmp / "work"
        git(self.tmp, "init", "--bare", "-b", "main", str(self.remote))
        git(self.tmp, "clone", str(self.remote), str(self.work))
        for k, v in (("user.name", "Test"), ("user.email", "t@example.com"), ("commit.gpgsign", "false")):
            git(self.work, "config", k, v)
        (self.work / "web").mkdir()
        (self.work / "web" / "progress.json").write_text('{"schema": 1, "translation": {"percent": 1.0}}\n', encoding="utf-8")
        git(self.work, "add", ".")
        git(self.work, "commit", "-m", "init")
        git(self.work, "push", "-u", "origin", "main")
        self.basic_project()
        self.app = self.new_app()
        self.app.ext = ExtensionHost()                       # the Pro pack, as a licensed installation has it
        self.app.ext.register(lexo_pro.provider())
        self.app.config.update({"web": {"repo": str(self.work), "path": "web/progress.json", "title": "T"}})
        self.head = lambda: git(self.remote, "rev-parse", "main")

    def test_preview_changes_nothing_anywhere(self):
        before = (self.head(), git(self.work, "rev-parse", "HEAD"), git(self.work, "status", "--porcelain"))
        info = self.app.publish_plan()
        self.app.export_preview()
        self.assertEqual((info["branch"], info["upstream"]), ("main", "origin/main"))
        self.assertTrue(info["changed"])
        self.assertTrue(any("deploys the website live" in w for w in render(info["warnings"])))
        self.assertIn("git push", info["commands"])
        self.assertEqual(before, (self.head(), git(self.work, "rev-parse", "HEAD"), git(self.work, "status", "--porcelain")))

    def test_editing_never_pushes_or_touches_the_repo(self):
        before = self.head()
        self.app.set_assign("script.rpy", "ep:1")
        self.app.save_file("script.rpy", "1", "")
        self.app.refresh(force=True)
        self.assertEqual(self.head(), before)
        self.assertEqual(git(self.work, "status", "--porcelain"), "")

    def test_publish_requires_current_token_and_branch(self):
        info = self.app.publish_plan()
        before = self.head()
        with self.assertRaises(AppError):
            self.app.publish_confirm("wrong", "main")
        with self.assertRaises(AppError):
            self.app.publish_confirm(info["token"], "develop")
        self.app.set_assign("script.rpy", "ep:1")          # data changed after the preview
        with self.assertRaises(AppError) as cm:
            self.app.publish_confirm(info["token"], "main")
        self.assertIn("changed since the preview", str(cm.exception))
        self.assertEqual(self.head(), before)

    def test_successful_publish_commits_only_the_data_file(self):
        (self.work / "notes.txt").write_text("keep me", encoding="utf-8")        # unrelated dirty file
        info = self.app.publish_plan()
        self.assertTrue(any("other uncommitted changes" in w for w in render(info["warnings"])))
        res = self.app.publish_confirm(info["token"], "main")
        self.assertTrue(res["ok"])
        self.assertEqual(git(self.remote, "show", "--name-only", "--format=%s", "main").splitlines(),
                         ["chore(translation): update progress data", "", "web/progress.json"])
        data = json.loads(git(self.remote, "show", "main:web/progress.json"))
        self.assertEqual(data["schema"], 2)
        self.assertEqual(data["seasons"][2], {"n": 3, "status": "planned"})
        self.assertEqual(git(self.work, "status", "--porcelain"), "?? notes.txt")
        self.assertEqual((self.work / "notes.txt").read_text(encoding="utf-8"), "keep me")

    def test_second_publish_without_changes_is_a_noop(self):
        info = self.app.publish_plan()
        self.app.publish_confirm(info["token"], "main")
        head = self.head()
        info2 = self.app.publish_plan()
        self.assertFalse(info2["changed"])
        res = self.app.publish_confirm(info2["token"], "main")
        self.assertIn("No change", render(res["message"]))
        self.assertEqual(self.head(), head)

    def test_failed_push_rolls_everything_back(self):
        hook = self.remote / "hooks" / "pre-receive"
        hook.write_text("#!/bin/sh\necho rejected by test >&2\nexit 1\n", encoding="utf-8", newline="\n")
        os.chmod(hook, 0o755)
        local_before = git(self.work, "rev-parse", "HEAD")
        file_before = (self.work / "web" / "progress.json").read_bytes()
        info = self.app.publish_plan()
        with self.assertRaises(AppError) as cm:
            self.app.publish_confirm(info["token"], "main")
        self.assertIn("push failed", str(cm.exception))
        self.assertEqual(git(self.work, "rev-parse", "HEAD"), local_before)
        self.assertEqual((self.work / "web" / "progress.json").read_bytes(), file_before)
        self.assertEqual(git(self.work, "status", "--porcelain"), "")

    def test_blockers_without_upstream_or_on_detached_head(self):
        git(self.work, "checkout", "-b", "feature")
        info = self.app.publish_plan()
        self.assertTrue(info["blockers"])
        with self.assertRaises(AppError):
            self.app.publish_confirm(info["token"], "feature")

    def test_target_path_validation(self):
        for bad in ("../x.json", "/abs/x.json", "C:/x.json", "web/progress.txt", ""):
            with self.assertRaises(pub.PublishError, msg=bad):
                pub.clean_target(bad)
        self.assertEqual(pub.clean_target("web\\progress.json"), "web/progress.json")
        with self.assertRaises(AppError):
            self.app.update_settings({"web": {"path": "../evil.json"}})

    def test_clone_case_is_previewed_without_network_or_disk_changes(self):
        self.app.config.update({"web": {"repo": "no-such-owner-xyz/no-such-repo-xyz", "path": "web/progress.json"}})
        info = self.app.publish_plan()
        self.assertTrue(info["will_clone"])
        self.assertFalse(Path(info["repo_path"]).exists())
        self.assertTrue(any("cloned" in w for w in render(info["warnings"])))
        with self.assertRaises(AppError):
            self.app.publish_confirm(info["token"], "main")         # clone needs its own confirmation marker

    def test_without_a_website_the_setup_message_comes_from_the_pack(self):
        self.app.config.update({"web": {"repo": "", "path": "web/progress.json"}})
        with self.assertRaises(AppError) as cm:
            self.app.publish_plan()
        self.assertIn("not set up", str(cm.exception))

    def test_the_pack_speaks_plain_json_and_the_interface_version_of_the_host(self):
        info = self.app.ext.call("publish.web", "plan", {"web": self.app.config.get("web", {}), "data": self.app.build_export(), "text": "{}"})
        json.dumps(info)                                      # plain JSON all the way
        self.assertEqual(lexo_pro.provider().api, API)
        self.assertEqual(self.app.ext.decide("publish.web").available, True)



if __name__ == "__main__":
    unittest.main()
