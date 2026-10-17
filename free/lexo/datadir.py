"""Where the data of a project lives (docs/DATA-LAYOUT.md).

  <translation folder>/.lexo/                 shared, small, text files that a team can put in git (CSV, JSON)
  <config folder>/projects/<project id>/      local data: backups, snapshots, the lock, text history, derived progress files

The shared folder holds a one-line file `project-id`; the local folder is named after it, so the local data follows the project when the
folder is moved or the game is renamed. Nothing bulky or private is written into the game folder.

"""
import secrets
from pathlib import Path

from .config import config_dir
from .util import atomic_write_text

SHARED_NAME = ".lexo"
ID_FILE = "project-id"

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
