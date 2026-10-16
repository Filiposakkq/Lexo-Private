"""One-time migration of the data folder from the Czech file names of the BaDIK tool to the English ones.

This is the only place that knows the old names. The old files are never changed or deleted: a new file is written only when it does
not exist yet, so running the migration twice (or opening the project with the old tool again) does no harm.
Only the header row and the Czech words used as values are translated; everything else is copied as it is.
"""
from pathlib import Path

from .messages import msg
from .util import WriteFailed, fold, read_rows, write_csv

# old name -> (new name, new header)
CSV_FILES = {
    "korekce.csv": ("proofread.csv", ["file", "proofread", "translated_manual"]),
    "prirazeni.csv": ("assignments.csv", ["file", "group"]),
    "schvaleno.csv": ("approved.csv", ["file", "block", "original"]),
    "epizody.csv": ("episode_states.csv", ["episode", "state"]),
    "korektura_radky.csv": ("line_marks.csv", ["file", "block", "original", "state", "hash", "note", "date"]),
    "ignorovano.csv": ("ignored.csv", ["file", "block", "original", "code", "hash", "note", "date"]),
    "slovnik.csv": ("glossary.csv", ["code", "name", "type", "note"]),
    "historie.csv": ("history.csv", ["date", "translated", "total", "percent", "proofread", "proofread_percent"]),
}
EPISODE_STATES = {"nezacato": "none", "rozpracovano": "wip", "prelozeno": "tr", "v korekci": "proof", "hotovo": "done"}   # as fold() writes them


def _assignment(value):
    f = fold(value)
    if f.startswith("obecn"):
        return "general"
    digits = "".join(ch for ch in f if ch.isdigit())
    return f"episode {int(digits)}" if digits else ""


def _convert(old, rows):
    """Rows of an old file (header included) -> rows of the new file (header included); values translated where needed."""
    body = rows[1:]
    if old == "prirazeni.csv":
        body = [[r[0], _assignment(r[1])] + r[2:] if len(r) > 1 else r for r in body]
    elif old == "epizody.csv":
        body = [[r[0], EPISODE_STATES.get(fold(r[1]), r[1])] + r[2:] if len(r) > 1 else r for r in body]
    return body


def run(data_dir):
    """Create the English-named files from the old ones. Returns a list of messages for the user (empty when there was nothing to do)."""
    data_dir = Path(data_dir)
    made, failed = [], []
    for old, (new, header) in CSV_FILES.items():
        src, dst = data_dir / old, data_dir / new
        if not src.is_file() or dst.exists():
            continue
        try:
            rows = read_rows(src)
            write_csv(dst, [header] + _convert(old, rows), data_dir / "backups")
            made.append(new)
        except (OSError, WriteFailed) as e:
            failed.append(msg("{file} could not be converted: {reason}", file=old, reason=e))
    notes = list(failed)
    if made:
        notes.insert(0, msg("The data files were copied to the new names ({files}). The old files were left untouched.", files=", ".join(made)))
    return notes
