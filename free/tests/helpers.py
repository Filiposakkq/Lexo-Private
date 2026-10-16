"""Shared test helpers: build small synthetic Ren'Py translation projects in a temp folder."""
import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from lexo import datadir, layout  # noqa: E402
from lexo import lang  # noqa: E402
from lexo.config import Config  # noqa: E402
from lexo.app import App  # noqa: E402

lang.use(lang.CS)      # tests that call the validator directly (no project open) use the Czech rules


def dialog(ident, who, src, tgt=None, line=10, suffix=""):
    """One `translate` block. tgt=None means untranslated (identical to the source)."""
    tgt = src if tgt is None else tgt
    return (f"# game/x.rpy:{line}\ntranslate czech {ident}:\n\n"
            f'    # {who} "{src}"{suffix}\n    {who} "{tgt}"{suffix}\n\n')


def strings(*pairs):
    out = "translate czech strings:\n\n"
    for old, new in pairs:
        out += f'    # game/s.rpy:1\n    old "{old}"\n    new "{new}"\n\n'
    return out


def rel(*parts):
    return os.path.join(*parts)


class TempCase(unittest.TestCase):
    """Creates a temp project folder and an isolated config folder for every test."""

    def setUp(self):
        self._tmp = tempfile.mkdtemp(prefix="lexo-test-")
        self.addCleanup(shutil.rmtree, self._tmp, ignore_errors=True)
        self.tmp = Path(self._tmp)
        self.root = self.tmp / "czech"
        self.root.mkdir()
        self.cfg_dir = self.tmp / "cfg"
        self._old_env = os.environ.get("LEXO_CONFIG_DIR")
        os.environ["LEXO_CONFIG_DIR"] = str(self.cfg_dir)
        self.addCleanup(self._restore_env)
        self.addCleanup(layout.use, layout.Layout(language="cs"))   # the older tests expect Czech rules
        self.use_layout(layout.BADIK_SECTIONS)

    def use_layout(self, sections, labels=None, language="cs"):
        """Write the project layout file (lexo.json) of the temp project; None = no file (no groups)."""
        path = self.root / ".lexo" / layout.FILE_NAME
        lay = layout.Layout(sections or (), labels, language)
        layout.use(lay)                      # tests that use Store/bulk directly never open a project
        if sections is None:
            path.unlink(missing_ok=True)
            return
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(lay.to_json()), encoding="utf-8")

    def local_dir(self):
        """The local data folder (backups, snapshots, lock, derived files) of the temp project."""
        shared = self.root / ".lexo"
        shared.mkdir(exist_ok=True)
        return datadir.local_dir(shared, self.root)

    def _restore_env(self):
        if self._old_env is None:
            os.environ.pop("LEXO_CONFIG_DIR", None)
        else:
            os.environ["LEXO_CONFIG_DIR"] = self._old_env

    def write(self, name, text, base=None):
        p = (base or self.root) / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8")
        return p

    def new_app(self, open_root=True):
        app = App(Config().load())
        if open_root:
            app.open_project(self.root)
        return app

    def basic_project(self):
        """Three files in the real project layout: ep1, a season-2 episode and a shared file."""
        self.write("script.rpy", dialog("a1", "mc", "Hello there, friend.", "Ahoj tam, příteli.", 10)
                   + dialog("a2", "mc", "Hmm...", None, 20)
                   + dialog("a3", "de", "I really do not know what to say.", None, 30))
        self.write(rel("scripts", "episode2", "ep2_party.rpy"),
                   dialog("b1", "mc", "Party time!", "Čas na párty!", 5) + dialog("b2", "mc", "Hmm...", None, 9))
        self.write(rel("season2", "scripts", "update5.rpy"),
                   dialog("c1", "mc", "Hmm...", None, 5) + dialog("c2", "de", "Welcome back.", "Vítej zpět.", 9))
        self.write("options.rpy", strings(("Start", "Začít"), ("Quit", "Quit")))
