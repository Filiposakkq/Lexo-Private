"""Event log: every change of the shared project state, one append-only file per author (docs/technical/DATA-LAYOUT.md, docs/product/TEAMWORK.md).

    <translation folder>/.aside/events/<author>.jsonl      one JSON object per line, written only by that author
    <config folder>/projects/<id>/events-seen.json         local: how many bytes of each file this computer has already read

An event says "this key of this kind now has this value" (`value` null = removed):

    {"v": 1, "t": "2026-10-17T09:12:00.123456+00:00", "a": "filip-3fa2", "kind": "proof", "key": ["script.rpy", "label_x", "Hello"], "value": {...}}

The state is the replay of all events of all authors; for one key the newest event wins (time, then author id, then position in the file).
The CSV files next to it stay as the cache that is read at start-up and edited by hand in Excel; the log and the CSV are reconciled by `Store.sync_events`.
Undo is a new event, never a deleted line. A damaged line is skipped. Nothing here knows about the kinds; `store.py` flattens its state into
`{kind: {key tuple: value}}` and applies replayed values back.
"""
import json
import re
import threading
from datetime import datetime, timedelta, timezone
from pathlib import Path

from .util import atomic_write_text

VERSION = 1
SEEN_FILE = "events-seen.json"


def clean_author(name):
    """A safe file name for an author id."""
    out = re.sub(r"[^a-z0-9_-]+", "-", (name or "").strip().lower()).strip("-")
    return out[:40] or "author"


class EventLog:
    def __init__(self, directory, seen_path, author):
        self.dir = Path(directory)
        self.seen_path = Path(seen_path)
        self.author = clean_author(author)
        self.path = self.dir / (self.author + ".jsonl")
        self.state = {}            # kind -> {key tuple: (value, stamp)}; the replay of everything read so far
        self.offsets = {}          # file name -> bytes already read (and applied to `state`)
        self.known = False         # was there a seen file (False = first run of this computer on this project)
        self._last_t = ""
        self._lock = threading.Lock()
        self._load_seen()

    # ------------------------------------------------------------ the local "seen" file
    def _load_seen(self):
        try:
            data = json.loads(self.seen_path.read_text(encoding="utf-8"))
            self.offsets = {str(k): int(v) for k, v in data.get("offsets", {}).items()}
            self.known = True
        except (OSError, ValueError, TypeError, AttributeError):
            self.offsets, self.known = {}, False

    def save_seen(self):
        try:
            atomic_write_text(self.seen_path, json.dumps({"v": VERSION, "offsets": self.offsets}, indent=1))
        except Exception:                     # a failed note only means more is read next time
            pass

    # ------------------------------------------------------------ reading
    def files(self):
        return sorted(self.dir.glob("*.jsonl")) if self.dir.is_dir() else []

    def _read_file(self, path, start=0):
        """[(event, end offset)] of the complete lines from `start`; a damaged line is skipped."""
        try:
            with open(path, "rb") as f:
                f.seek(start)
                data = f.read()
        except OSError:
            return [], start
        cut = data.rfind(b"\n") + 1
        out, pos = [], start
        for raw in data[:cut].split(b"\n")[:-1] if cut else []:
            pos += len(raw) + 1
            try:
                ev = json.loads(raw.decode("utf-8"))
                if isinstance(ev, dict) and ev.get("kind") and isinstance(ev.get("key"), list) and isinstance(ev.get("t"), str):
                    out.append((ev, pos))
            except ValueError:
                continue
        return out, start + cut

    def read_all(self):
        """Every event of every file, split into (seen, unseen) by the saved offsets; sorted nowhere (replay sorts)."""
        seen, unseen, ends = [], [], {}
        for path in self.files():
            evs, end = self._read_file(path, 0)
            limit = self.offsets.get(path.name, 0) if self.known else 0
            for ev, pos in evs:
                (seen if pos <= limit else unseen).append((ev, path.name))
            ends[path.name] = end
        return seen, unseen, ends

    def read_new(self):
        """Events written since the last read (any author); advances the offsets."""
        fresh, ends = [], {}
        for path in self.files():
            evs, end = self._read_file(path, self.offsets.get(path.name, 0))
            fresh += [(ev, path.name) for ev, _ in evs]
            ends[path.name] = end
        self.offsets.update(ends)
        return fresh

    # ------------------------------------------------------------ replay
    @staticmethod
    def _stamp(ev, source, n):
        return (ev.get("t", ""), str(ev.get("a", source)), n)

    def replay(self, items, state=None):
        """Merge [(event, file name)] into `state` (newest wins per key); returns the state."""
        state = self.state if state is None else state
        for n, (ev, source) in enumerate(items):
            stamp = self._stamp(ev, source, n)
            try:
                key = tuple(ev["key"])
                hash(key)
            except TypeError:
                continue
            slot = state.setdefault(str(ev["kind"]), {})
            old = slot.get(key)
            if old is None or stamp >= old[1]:
                slot[key] = (ev.get("value"), stamp)
        return state

    def flat(self, state=None):
        """{kind: {key: value}} without removed keys."""
        state = self.state if state is None else state
        return {k: {key: v for key, (v, _) in slot.items() if v is not None} for k, slot in state.items()}

    # ------------------------------------------------------------ writing
    def _now(self):
        t = datetime.now(timezone.utc)
        s = t.isoformat(timespec="microseconds")
        if s <= self._last_t:                  # strictly increasing within this process
            s = (datetime.fromisoformat(self._last_t) + timedelta(microseconds=1)).isoformat(timespec="microseconds")
        self._last_t = s
        return s

    def append(self, changes):
        """Write [(kind, key tuple, value or None)] as events of this author and merge them into the state. Returns the number written."""
        if not changes:
            return 0
        with self._lock:
            self.dir.mkdir(parents=True, exist_ok=True)
            lines, items = [], []
            for kind, key, value in changes:
                ev = {"v": VERSION, "t": self._now(), "a": self.author, "kind": kind, "key": list(key), "value": value}
                lines.append(json.dumps(ev, ensure_ascii=False, separators=(",", ":")))
                items.append((ev, self.path.name))
            lead = b""
            try:
                if self.path.stat().st_size and not self.path.read_bytes().endswith(b"\n"):
                    lead = b"\n"                          # a previous write was cut short: never glue two lines
            except OSError:
                pass
            blob = lead + ("\n".join(lines) + "\n").encode("utf-8")
            with open(self.path, "ab") as f:
                f.write(blob)
            self.replay(items)
            self.offsets[self.path.name] = self.path.stat().st_size
            self.save_seen()
            return len(lines)
