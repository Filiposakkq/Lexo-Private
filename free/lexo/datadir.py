"""Where the data of a project lives (docs/DATA-LAYOUT.md).

  <translation folder>/.lexo/                 shared, small, text files that a team can put in git (CSV, JSON)
  <config folder>/projects/<project id>/      local data: backups, snapshots, the lock, text history, derived progress files

The shared folder holds a one-line file `project-id`; the local folder is named after it, so the local data follows the project when the
folder is moved or the game is renamed. Nothing bulky or private is written into the game folder.

Data of older versions is copied (never moved, never deleted) the first time a project is opened: the shared files from `_progress_data`
(`project.json` becomes `lexo.json`), the local ones to the local folder. The old folder stays as it was.
"""
import secrets
import shutil
from pathlib import Path

from .config import config_dir
from .messages import msg
from .util import atomic_write_text

SHARED_NAME = ".lexo"
LEGACY_NAME = "_progress_data"
ID_FILE = "project-id"

# What stays in the old folder's shared part and is copied; the rest of the top-level files are local or derived.
LOCAL_DIRS = ("backups", "snapshots")
LOCAL_FILES = ("historie_textu.jsonl", "text_history.jsonl", "last_fix.json", "lock.json")
SKIP_FILES = ("progress.csv", "progress.json", "lock.json", "last_fix.json", "historie_textu.jsonl", "text_history.jsonl")
RENAMED = {"project.json": "lexo.json"}
LOCAL_RENAMED = {"historie_textu.jsonl": "text_history.jsonl"}


def project_id(shared):
    """The id stored in the shared folder; created on first use. Returns '' when the folder cannot be written."""
    path = Path(shared) / ID_FILE
    try:
        pid = path.read_text(encoding="utf-8").strip()
        if pid.isalnum() and len(pid) >= 8:
            return pid
    except OSError:
        pass
    pid = secrets.token_hex(6)
    try:
        atomic_write_text(path, pid + "\n")
    except Exception:                                   # a read-only folder: the caller falls back to the folder name
        return ""
    return pid


def local_dir(shared, root):
    """The local data folder of the project (created)."""
    pid = project_id(shared) or "path-" + "".join(c if c.isalnum() else "_" for c in str(Path(root).resolve()))[-40:]
    d = config_dir() / "projects" / pid
    d.mkdir(parents=True, exist_ok=True)
    return d


def migrate_folder(root, shared, local):
    """Copy the data of a `_progress_data` folder into the new places once. Returns messages for the user."""
    old = Path(root) / LEGACY_NAME
    if not old.is_dir():
        return []
    copied, failed = 0, []
    for src in old.iterdir():
        try:
            if src.is_file() and src.name not in SKIP_FILES:
                dst = Path(shared) / RENAMED.get(src.name, src.name)
            elif src.is_file() and src.name in LOCAL_FILES:
                dst = Path(local) / LOCAL_RENAMED.get(src.name, src.name)
            elif src.is_dir() and src.name in LOCAL_DIRS:
                dst = Path(local) / src.name
            else:
                continue
            if dst.exists():                                    # never overwrite what the new folders already have
                continue
            shutil.copytree(src, dst) if src.is_dir() else shutil.copy2(src, dst)
            copied += 1
        except OSError as e:
            failed.append(msg("{file} could not be copied: {reason}", file=src.name, reason=e))
    notes = list(failed)
    if copied:
        notes.insert(0, msg("The data of the project was copied from {old} to {new}. The old folder was left untouched and can be deleted when you no longer need it.",
                            old=LEGACY_NAME, new=SHARED_NAME))
    return notes
