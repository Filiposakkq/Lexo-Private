"""Golden data of Core: fixed inputs and the answers Core gives to them, for checking another implementation (the Rust port) against this one.

    py core/tests/golden_tool.py              rewrite core/golden/expected/ from the inputs (after a change that is meant to change an answer)

The inputs are plain files under `core/golden/` (the source of truth, committed): `.rpy` cases for the parser, `.rpa` archives, cases for the layout proposals, the
menu finder and the validator, whole projects (`projects/<name>/game/...`) and scenarios (`scenarios/<name>.json`: a project and a list of operation calls).
The answers are in `core/golden/expected/` (JSON, normalised: no absolute paths, no times, no random ids). `test_golden.py` checks that Core still gives exactly them.
A port to another language implements the same small runner (read the inputs, call, normalise, compare), see docs/GOLDEN.md.
"""
import base64
import hashlib
import json
import os
import re
import shutil
import sys
import tempfile
from dataclasses import asdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path[:0] = [str(HERE.parent), str(HERE)]

from lexo import api, layout, lang, menus, rpa, rpy, validator    # noqa: E402
from lexo.app_base import AppError                                   # noqa: E402
from lexo.messages import render, text_of, wire                      # noqa: E402
from lexo.fixes import FixError                                     # noqa: E402
from lexo.project import ProjectUnavailable                         # noqa: E402
from lexo.snapshots import SnapshotError                            # noqa: E402
from lexo.util import WriteFailed                                   # noqa: E402

GOLDEN = ROOT / "core" / "golden"
EXPECTED = GOLDEN / "expected"
VOLATILE_KEYS = ("time", "date", "updated", "elapsed", "token", "started", "mtime", "created", "stamp", "loaded_at", "seconds", "author")
PATH_PARAMS = ("file", "name", "files", "names")                    # parameters that name a translation file: written with "/" in a scenario, with the separator of the system for the call
REL_RPY = re.compile(r"^[\w .\-]+(?:\\[\w .\-]+)+\.rpy$")           # beach\a.rpy (a file in a folder, as Windows writes it)
LANGS = {"cs": lang.CS, "neutral": lang.NEUTRAL}


def load_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def dump(path, obj):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(obj, ensure_ascii=False, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")


def plain(value):
    """What a client sees: messages as their English text, everything plain JSON."""
    return render(json.loads(json.dumps(wire(value), default=str)))


# =========================================================================================================================== the parser
def parse_case(path, language):
    lang.use(LANGS[language])
    parsed = rpy.parse_bytes(Path(path).read_bytes())
    return plain({
        "entries": [{**asdict(e), "pending": e.pending, "key": list(e.key)} for e in parsed.entries],
        "block_ids": [list(b) for b in parsed.block_ids],
        "structure": [list(s) for s in parsed.structure],
        "digest": parsed.digest,
        "languages": sorted(parsed.languages),
        "counts": list(rpy.count(parsed.entries)),
    })


def run_rpy():
    cases, out = load_json(GOLDEN / "rpy" / "cases.json"), {}
    for name, spec in sorted(cases.items()):
        if name.startswith("_"):
            continue
        out[name] = parse_case(GOLDEN / "rpy" / "cases" / spec.get("file", name + ".rpy"), spec["language"])
    lang.use(lang.CS)
    return out


# =========================================================================================================================== archives
def run_rpa():
    out = {}
    for path in sorted((GOLDEN / "rpa" / "archives").glob("*.rpa")):
        try:
            version, index = rpa.read_index(path)
        except rpa.RpaError as e:
            out[path.stem] = {"error": str(text_of(e)).split(":")[0].split(" blocked ")[0]}          # the rest is the wording of Python's own pickle / zlib errors
            continue
        out[path.stem] = {"version": version, "files": {
            name: {"offset": o, "length": n, "prefix": p.hex(), "sha1": hashlib.sha1(rpa.read_entry(path, (o, n, p))).hexdigest()}
            for name, (o, n, p) in sorted(index.items())}}
    return out


# =========================================================================================================================== layout, menus, validator
def run_layout():
    cases = load_json(GOLDEN / "layout" / "cases.json")
    out = {"propose": {c["name"]: plain(layout.propose(c["files"])) for c in cases["propose"]}}
    fn = cases["folder_number"]
    out["folder_number"] = {p: layout.folder_number(p, fn["folders"]) for p in fn["paths"]}
    sn = cases["series_number"]
    out["series_number"] = {p: layout.series_number(p, sn["series"]) for p in sn["paths"]}
    return out


def run_menus():
    out = []
    for c in load_json(GOLDEN / "menus" / "cases.json")["cases"]:
        lines = (GOLDEN / "menus" / "scripts" / c["script"]).read_text(encoding="utf-8").splitlines()
        fn = menus.menu_around if c.get("around") else menus.find_menu
        out.append({"script": c["script"], "line": c["line"], "choice": c["choice"], "around": bool(c.get("around")), "result": plain(fn(lines, c["line"], c["choice"]))})
    return out


def run_validator():
    lang.use(lang.CS)
    out = []
    for c in load_json(GOLDEN / "validator" / "cases.json")["cases"]:
        e = rpy.Entry(line=1, src_line=1, block="translate czech x", unit="dialogue", who=c.get("who", ""), suffix=c.get("suffix", ""), source=c["source"], target=c["target"],
                      src_who=c.get("src_who", c.get("who", "")), src_suffix=c.get("src_suffix", c.get("suffix", "")),
                      src_raw=f'{c.get("src_who", c.get("who", ""))} "{c["source"]}"'.strip(), cur_raw=f'{c.get("who", "")} "{c["target"]}"'.strip(),
                      category=rpy.classify(c["target"]) if c["source"] == c["target"] or not c["target"] else "")
        issues = validator.check_entry(e)
        out.append({"name": c["name"], "pending": e.pending, "category": e.category, "issues": [{"code": i.code, "severity": i.severity, "category": i.category} for i in issues]})
    return out


# =========================================================================================================================== spelling hints, find / replace, font metrics
def run_spelling():
    from lexo import spelling
    spec = load_json(GOLDEN / "spelling" / "cases.json")
    sp = spelling.Spelling.build(spec["targets"], spec["known"])
    suggest = {}
    for w in spec["suggest"]:
        hit = sp.suggest(w)
        suggest[w] = list(hit) if hit else None
    check = [{"name": c["name"], "issues": [plain(i.to_dict()) for i in sp.check(c["source"], c["target"])]} for c in spec["check"]]
    return {"sig": sp.sig, "vocab_size": len(sp.vocab), "vocab_total": sum(sp.vocab.values()), "by_fold": dict(sorted(sp.by_fold.items())), "suggest": suggest, "check": check}


def run_textedit():
    from lexo import textedit
    spec = load_json(GOLDEN / "textedit" / "cases.json")
    out = {"cases": [], "case_like": [textedit.case_like(m, n) for m, n in spec["case_like"]], "mask": [textedit.mask(t) for t in spec["mask"]]}
    for c in spec["cases"]:
        entry = {"name": c["name"]}
        try:
            fn = textedit.make_replacer(c["find"], c["replace"], c["mode"], c.get("case", False))
            entry["result"] = list(fn(c["text"], c.get("protect", True)))
        except ValueError as e:
            entry["error"] = str(text_of(e))
        out["cases"].append(entry)
    return out


def run_fontmetrics():
    from lexo import fontmetrics
    out = {}
    for path in sorted((GOLDEN / "fonts").iterdir()):
        got = fontmetrics.advances(path.read_bytes())
        out[path.name] = None if got is None else {"upm": got["upm"], "adv": {str(k): v for k, v in sorted(got["adv"].items())}}
    return out


# =========================================================================================================================== scenarios on whole projects
def _str(value, roots):
    for r in roots:
        value = value.replace(r, "<TMP>")
    if "<TMP>" in value:
        value = value.replace("\\", "/")
    value = re.sub(r"projects/[0-9a-f]{6,}", "projects/<id>", value)           # the folder of the local data is named by the id of the project (random)
    value = re.sub(r"snap-\d{8}-\d{6}", "snap-<stamp>", value)
    value = re.sub(r"\.\d{8}-\d{6}-\d{6}\.bak", ".<stamp>.bak", value)
    return "/".join(value.split("\\")) if REL_RPY.match(value) else value


def normalize(value, roots):
    """Plain JSON without what differs between runs or systems: the temporary folders, times, random ids, the separator of a path to a translation file."""
    if isinstance(value, str):
        return _str(value, roots)
    if isinstance(value, list):
        return [normalize(v, roots) for v in value]
    if isinstance(value, dict):
        return {_str(k, roots): ("<volatile>" if any(w in k.lower() for w in VOLATILE_KEYS) and not isinstance(v, (dict, list)) else normalize(v, roots)) for k, v in value.items()}
    return value


def native(params):
    """The parameters of a step with the path separator of this system in the names of files."""
    def fix(v):
        return v.replace("/", os.sep) if isinstance(v, str) else [fix(x) for x in v] if isinstance(v, list) else v
    return {k: (fix(v) if k in PATH_PARAMS else v) for k, v in params.items()}


def _roots(base):
    out = set()
    for p in (base, base.resolve()):
        for s in (str(p), str(p).replace("\\", "/"), str(p).replace("\\", "\\\\")):
            out.add(s)
    return sorted(out, key=len, reverse=True)


def run_scenario(path):
    from lexo.app import App
    from lexo.config import Config
    spec = load_json(path)
    tmp = Path(tempfile.mkdtemp(prefix="lexo-golden-"))
    old_cfg = os.environ.get("LEXO_CONFIG_DIR")
    os.environ["LEXO_CONFIG_DIR"] = str(tmp / "cfg")
    base = tmp / "p"
    try:
        shutil.copytree(GOLDEN / "projects" / spec["project"], base)
        tl = base / spec.get("tl", "game/tl/czech")
        app = App(Config().load())
        steps = []
        try:
            app.open_project(tl)
            for st in spec["steps"]:
                entry = {"op": st["op"], "params": st.get("params", {})}
                try:
                    entry["answer"] = plain(api.call(app, st["op"], native(st.get("params", {}))))
                except (AppError, KeyError) as e:
                    entry["error"] = str(text_of(e)) if isinstance(e, AppError) else f"missing parameter {e}"
                steps.append(entry)
            files = {}
            for f in sorted(tl.rglob("*")):
                if f.is_file() and f.suffix == ".rpy":
                    b = f.read_bytes()
                    rec = {"sha1": hashlib.sha1(b).hexdigest()}
                    try:
                        rec["text"] = b.decode("utf-8")
                    except UnicodeDecodeError:
                        rec["base64"] = base64.b64encode(b).decode("ascii")
                    files[f.relative_to(tl).as_posix()] = rec
        finally:
            app.close()
        return normalize({"project": spec["project"], "steps": steps, "files_after": files}, _roots(tmp))
    finally:
        if old_cfg is None:
            os.environ.pop("LEXO_CONFIG_DIR", None)
        else:
            os.environ["LEXO_CONFIG_DIR"] = old_cfg
        shutil.rmtree(tmp, ignore_errors=True)


def scenario_names():
    return sorted(p.stem for p in (GOLDEN / "scenarios").glob("*.json"))


def data_names():
    return sorted(p.stem for p in (GOLDEN / "data").glob("*.json"))


# =========================================================================================================================== data files (docs/GOLDEN.md "data scenarios")
class _Clock:
    """The clock of a data scenario: local and UTC time fixed (so the names and lines Core writes are the same in every run), the epoch real but shifted
    by `advance` steps (file times are real, so only a shift lets the "is the newest backup old enough" rule be tested), or fixed by `set_epoch`."""

    def __init__(self, spec):
        from datetime import datetime, timezone
        self.local = datetime.fromisoformat(spec.get("local", "2026-10-17T11:00:00"))
        self.utc = datetime.fromisoformat(spec.get("utc", "2026-10-17T09:00:00+00:00")).astimezone(timezone.utc)
        self.offset, self.fixed = 0.0, None

    def epoch(self):
        import time as _t
        return self.fixed if self.fixed is not None else _t.time() + self.offset


def _patches(clock):
    """Replace the clocks Core reads: `datetime.now()` of the modules that stamp their files, `time.time()` where an age is measured."""
    from datetime import datetime
    from types import SimpleNamespace
    from unittest import mock
    import time as _t

    class FakeDT(datetime):
        @classmethod
        def now(cls, tz=None):
            if tz is None:
                loc = clock.local
                return cls(loc.year, loc.month, loc.day, loc.hour, loc.minute, loc.second, loc.microsecond)
            u = clock.utc
            return cls(u.year, u.month, u.day, u.hour, u.minute, u.second, u.microsecond, tzinfo=u.tzinfo).astimezone(tz)

    fake_time = SimpleNamespace(time=clock.epoch, sleep=_t.sleep)
    stack = [mock.patch(f"lexo.{m}.datetime", FakeDT) for m in ("util", "events", "texthistory", "snapshots", "fixes")]
    stack += [mock.patch("lexo.util.time", fake_time), mock.patch("lexo.lock.time", fake_time)]
    return stack


def _file_records(root, skip=()):
    files = {}
    for f in sorted(root.rglob("*")):
        if f.is_file() and f.name not in skip:
            b = f.read_bytes()
            if f.name == "text_history.jsonl":
                b = b.replace(b"\r\n", b"\n")                  # Python writes it in text mode: CRLF on Windows, LF elsewhere
            if f.name == "last_fix.json":                      # it holds absolute paths of backups: compared as data, after the paths were normalised
                files[f.relative_to(root).as_posix()] = {"json": json.loads(b.decode("utf-8"))}
                continue
            if f.suffix == ".zip":                             # the bytes of a zip depend on the compressor: its files and their digests are compared
                import zipfile
                with zipfile.ZipFile(f) as z:
                    files[f.relative_to(root).as_posix()] = {"zip": sorted([i.filename, hashlib.sha1(z.read(i)).hexdigest()] for i in z.infolist())}
                continue
            rec = {"sha1": hashlib.sha1(b).hexdigest()}
            try:
                rec["text"] = b.decode("utf-8")
            except UnicodeDecodeError:
                rec["base64"] = base64.b64encode(b).decode("ascii")
            files[f.relative_to(root).as_posix()] = rec
    return files


DATA_SKIP = ("lock.json", "project-id")          # random by design: a token, a time, a project id


def run_data_scenario(path):
    spec = load_json(path)
    tmp = Path(tempfile.mkdtemp(prefix="lexo-golden-"))
    old_cfg = os.environ.get("LEXO_CONFIG_DIR")
    os.environ["LEXO_CONFIG_DIR"] = str(tmp / "cfg")
    from contextlib import ExitStack
    try:
        root = tmp
        for rel, content in spec.get("files", {}).items():
            p = root / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(base64.b64decode(content["base64"]) if isinstance(content, dict) else content.encode("utf-8"))
        clock = _Clock(spec.get("clock", {}))
        layout.use(layout.Layout())                         # the layout is process-wide state of the Python module: every scenario starts without one
        runner = DataRunner(root, tmp / "cfg", clock)
        steps = []
        with ExitStack() as stack:
            for p in _patches(clock):
                stack.enter_context(p)
            for st in spec["steps"]:
                entry = {"op": st["op"], "params": {k: v for k, v in st.items() if k != "op"}}
                try:
                    entry["answer"] = plain(runner.run(st["op"], st))
                except (AppError, WriteFailed, SnapshotError, FixError, ProjectUnavailable) as e:
                    entry["error"] = str(text_of(e))
                steps.append(entry)
        runner.close()
        return normalize({"steps": steps, "files_after": _file_records(tmp, DATA_SKIP)}, _roots(tmp))
    finally:
        if old_cfg is None:
            os.environ.pop("LEXO_CONFIG_DIR", None)
        else:
            os.environ["LEXO_CONFIG_DIR"] = old_cfg
        shutil.rmtree(tmp, ignore_errors=True)


class DataRunner:
    """The steps of a data scenario: each one calls one function of Core with plain parameters and returns plain JSON."""

    def __init__(self, root, cfg_dir, clock):
        self.root, self.cfg_dir, self.clock = root, cfg_dir, clock
        self.locks = {}
        self.config = None

    def p(self, rel):
        return self.root / rel

    def close(self):
        for lk in self.locks.values():
            lk.release()

    def run(self, op, a):
        return getattr(self, "op_" + op.replace(".", "_"))(a)

    # -- time
    def op_advance(self, a):
        self.clock.offset += a["seconds"]
        return True

    def op_set_epoch(self, a):
        self.clock.fixed = a["value"]
        return True

    # -- util: files and CSV
    def op_csv_read(self, a):
        from lexo import util
        return util.read_rows(self.p(a["path"]))

    def op_csv_text(self, a):
        from lexo import util
        return util.csv_text(a["rows"], a.get("delimiter", ";"))

    def op_write_csv(self, a):
        from lexo import util
        util.write_csv(self.p(a["path"]), a["rows"], self.p(a["backup_dir"]) if a.get("backup_dir") else None)
        return True

    def op_atomic_write(self, a):
        from lexo import util
        util.atomic_write_text(self.p(a["path"]), a["text"], newline=a.get("newline"), backup_dir=self.p(a["backup_dir"]) if a.get("backup_dir") else None)
        return True

    def op_make_backup(self, a):
        from lexo import util
        got = util.make_backup(self.p(a["path"]), self.p(a["backup_dir"]), a.get("keep", 20), a.get("min_interval", 300))
        return got.name if got else None

    def op_list_dir(self, a):
        d = self.p(a["path"])
        return sorted(f.name for f in d.iterdir()) if d.is_dir() else None

    def op_text_match(self, a):
        from lexo import util
        fn = util.text_matcher(a["query"], a.get("whole", False))
        return None if fn is None else [fn(t) for t in a["texts"]]

    # -- config
    def op_config_load(self, a):
        from lexo.config import Config
        self.config = Config().load()
        return {"data": self.config.data, "load_error": str(self.config.load_error).split(" (")[0], "public": self.config.public()}

    def op_config_set(self, a):
        self.config.set(a["key"], a["value"])
        return self.config.get(a["key"])

    def op_config_get(self, a):
        return self.config.get(a["key"], a.get("default"))

    def op_config_update(self, a):
        self.config.update(a["patch"])
        return self.config.data

    # -- data folders
    def op_datadir_project_id(self, a):
        from lexo import datadir
        pid = datadir.project_id(self.p(a["shared"]))
        return {"ok": pid.isalnum() and len(pid) >= 8, "again": datadir.project_id(self.p(a["shared"])) == pid}

    def op_datadir_local_dir(self, a):
        from lexo import datadir
        d = datadir.local_dir(self.p(a["shared"]), self.p(a["root"]))
        return str(d)

    # -- lock
    def op_lock_acquire(self, a):
        from lexo import lock
        from unittest import mock
        lk = self.locks.get(a["name"]) or lock.ProjectLock(self.p(a["dir"]))
        if "token" in a:
            lk.token = a["token"]
        with mock.patch("os.getpid", lambda: a.get("pid", 1)), mock.patch("socket.gethostname", lambda: a.get("host", "pc")):
            try:
                lk.acquire(force=a.get("force", False))
            except lock.ProjectLocked as e:
                return {"locked": True, "holder": e.holder, "message": str(text_of(e))}
        self.locks[a["name"]] = lk
        return {"locked": False}

    def op_lock_check(self, a):
        return self.locks[a["name"]].check()

    def op_lock_release(self, a):
        lk = self.locks.pop(a["name"])
        lk.release()
        return not lk.held

    # -- layout, store and the event log
    def op_layout_use(self, a):
        from lexo import layout
        lay = layout.parse(a["raw"])
        layout.use(lay)
        return lay.to_json()

    def op_store_open(self, a):
        from lexo.store import Store
        self.store = Store(self.p(a["shared"]), self.p(a["local"]))
        return True

    def op_store_load(self, a):
        self.store.load()
        return self._dump_store()

    def op_store_dump(self, a):
        return self._dump_store()

    def _dump_store(self):
        st = self.store
        return {
            "corr": st.corr, "assign": st.assign,
            "approved": {f: sorted([list(k) for k in v]) for f, v in st.approved.items()},
            "episodes": {str(n): s for n, s in st.episodes.items()},
            "proof": {f: [[b, o, dict(m)] for (b, o), m in sorted(t.items())] for f, t in st.proof.items()},
            "ignored": {f: [[b, o, c, dict(m)] for (b, o, c), m in sorted(t.items())] for f, t in st.ignored.items()},
            "glossary": st.glossary, "warnings": list(st.warnings), "unsaved": sorted(st.unsaved), "undo": [s["label"] for s in st.undo_stack]}

    def _change(self, kind, fn):
        self.store.change(kind, [kind], fn)
        return self._dump_store()

    def op_store_assign(self, a):
        st = self.store
        return self._change("assign", lambda: st.assign.pop(a["file"], None) if a["value"] == "" else st.assign.__setitem__(a["file"], a["value"]))

    def op_store_corr(self, a):
        st = self.store
        return self._change("corr", lambda: st.corr.__setitem__(a["file"], {"c": a["c"], "t": a["t"]}))

    def op_store_approve(self, a):
        from lexo.util import norm_ws
        st = self.store

        def fn():
            if a["on"]:
                st.approved.setdefault(a["file"], set()).add((a["block"], norm_ws(a["original"])))
            else:
                st.approved.get(a["file"], set()).discard((a["block"], norm_ws(a["original"])))
                if not st.approved.get(a["file"], True):
                    del st.approved[a["file"]]
        return self._change("approved", fn)

    def op_store_episode(self, a):
        st = self.store
        return self._change("episodes", lambda: st.episodes.pop(a["n"], None) if a["state"] is None else st.episodes.__setitem__(a["n"], a["state"]))

    def _marks(self, kind, a, key):
        table = getattr(self.store, kind)

        def fn():
            if a["mark"] is None:
                table.get(a["file"], {}).pop(key, None)
                if not table.get(a["file"], True):
                    del table[a["file"]]
            else:
                table.setdefault(a["file"], {})[key] = dict(a["mark"])
        return self._change(kind, fn)

    def op_store_proof(self, a):
        from lexo.util import norm_ws
        return self._marks("proof", a, (a["block"], norm_ws(a["original"])))

    def op_store_ignored(self, a):
        from lexo.util import norm_ws
        return self._marks("ignored", a, (a["block"], norm_ws(a["original"]), a["code"]))

    def op_store_glossary(self, a):
        st = self.store
        return self._change("glossary", lambda: st.glossary.pop(a["code"], None) if a.get("remove") else st.glossary.__setitem__(a["code"], {"n": a["n"], "t": a["t"], "c": a["c"]}))

    def op_store_undo(self, a):
        label = self.store.undo()
        return {"label": label, "state": self._dump_store()}

    def op_store_ensure_files(self, a):
        self.store.ensure_files(a["names"])
        return self._dump_store()

    def op_store_attach_events(self, a):
        from lexo.events import EventLog
        self.store.attach_events(EventLog(self.p(a["dir"]), self.p(a["seen"]), a["author"]))
        return True

    def op_store_sync_events(self, a):
        merged = self.store.sync_events()
        return {"merged": merged, "state": self._dump_store(), "offsets": self.store.events.offsets, "known": self.store.events.known}

    def op_store_funcs(self, a):
        from lexo import store
        return {"parse_value": [store.parse_value(v, a["total"]) for v in a["values"]], "parse_corr": [store.parse_corr(v, a["total"]) for v in a["values"]],
                "text_hash": [store.text_hash(t) for t in a["texts"]],
                "assign_from_text": [store.assign_from_text(t) for t in a["assign_texts"]], "assign_to_text": [store.assign_to_text(v) for v in a["assigns"]],
                "valid_assign": [store.valid_assign(v) for v in a["assigns"]], "assign_label": [store.assign_label(v) for v in a["assigns"]],
                "group_label": [store.group_label(n) for n in a["groups"]], "ep_states": [[k, v] for k, v in store.ep_states()],
                "ep_of": [store.ep_of(v) for v in a["assigns"]]}

    def op_events_open(self, a):
        from lexo.events import EventLog
        self.log = EventLog(self.p(a["dir"]), self.p(a["seen"]), a["author"])
        return {"author": self.log.author, "known": self.log.known, "offsets": self.log.offsets}

    def op_events_append(self, a):
        return self.log.append([(k, tuple(key), v) for k, key, v in a["changes"]])

    def op_events_dump(self, a):
        seen, unseen, ends = self.log.read_all()
        return {"state": {kind: {json.dumps(list(key)): [v, list(stamp)] for key, (v, stamp) in slot.items()} for kind, slot in self.log.state.items()},
                "flat": {kind: {json.dumps(list(key)): v for key, v in slot.items()} for kind, slot in self.log.flat().items()},
                "offsets": self.log.offsets, "known": self.log.known,
                "read_all": {"seen": [[e, n] for e, n in seen], "unseen": [[e, n] for e, n in unseen], "ends": ends}}

    def op_events_replay_all(self, a):
        seen, unseen, ends = self.log.read_all()
        self.log.state = {}
        self.log.replay(seen)
        self.log.replay(unseen)
        return len(seen), len(unseen)

    def op_write_file(self, a):
        p = self.p(a["path"])
        p.parent.mkdir(parents=True, exist_ok=True)
        with open(p, "ab" if a.get("append") else "wb") as f:
            f.write(a["text"].encode("utf-8"))
        return True

    def op_events_clean_author(self, a):
        from lexo.events import clean_author
        return [clean_author(n) for n in a["names"]]

    # -- history.csv, the history of the texts, the translation memory, snapshots
    def op_set_local(self, a):
        from datetime import datetime
        self.clock.local = datetime.fromisoformat(a["value"])
        return True

    def op_remove_file(self, a):
        self.p(a["path"]).unlink()
        return True

    def op_history_open(self, a):
        from lexo.history import History
        self.hist = History(self.p(a["path"]), self.p(a["backup_dir"]) if a.get("backup_dir") else None)
        return True

    def op_history_record(self, a):
        return self.hist.record(a["stamp"], a["t_done"], a["total"], a["tpct"], a["c_done"], a["cpct"])

    def op_history_dump(self, a):
        return {"rows": self.hist.rows(), "points": [[str(ts), d, t, c] for ts, d, t, c in self.hist.points()],
                "chart": [[str(ts), x, y] for ts, x, y in self.hist.chart_points()]}

    @staticmethod
    def _dates(v):
        from datetime import date
        if isinstance(v, date):
            return v.isoformat()
        if isinstance(v, dict):
            return {k: DataRunner._dates(x) for k, x in v.items()}
        if isinstance(v, (list, tuple)):
            return [DataRunner._dates(x) for x in v]
        return v

    def _points(self, a):
        from datetime import datetime
        from lexo.history import STAMP_FORMAT
        if "points" in a:
            return [(datetime.strptime(ts, STAMP_FORMAT), d, t, c) for ts, d, t, c in a["points"]]
        return self.hist.points()

    def op_history_estimate(self, a):
        from datetime import date
        from lexo import history
        return self._dates(history.estimate(self._points(a), a["done_now"], a["total_now"], today=date.fromisoformat(a["today"])))

    def op_history_goals(self, a):
        from datetime import date
        from lexo import history
        return self._dates(history.goals(self._points(a), a["remaining_translate"], a["remaining_proof"], a.get("cfg"), date.fromisoformat(a["today"])))

    def op_texthistory_open(self, a):
        from lexo.texthistory import TextHistory
        self.th = TextHistory(self.p(a["path"]))
        return True

    def _dump_th(self):
        return {"total": self.th.total, "by_key": [[f, b, s, recs] for (f, b, s), recs in sorted(self.th.by_key.items())]}

    def op_texthistory_load(self, a):
        from unittest import mock
        from lexo import texthistory as t
        with mock.patch.object(t, "MAX_LINES", a.get("max_lines", t.MAX_LINES)), mock.patch.object(t, "KEEP_LINES", a.get("keep_lines", t.KEEP_LINES)):
            self.th.load()
        return self._dump_th()

    def op_texthistory_record(self, a):
        n = self.th.record([{"file": i["file"], "key": tuple(i["key"]), "old": i["old"], "new": i["new"]} for i in a["items"]], a["via"])
        return {"written": n, "state": self._dump_th()}

    def op_texthistory_versions(self, a):
        return self.th.versions(a["file"], a["key"], a["current"])

    def op_texthistory_diff(self, a):
        from lexo import rpy, texthistory
        old, new = rpy.parse_text(a["old"]).entries, rpy.parse_text(a["new"]).entries
        return [{**d, "key": list(d["key"])} for d in texthistory.diff_files(old, new)]

    def op_memory_build(self, a):
        from lexo.memory import Memory
        rows = [tuple(r) for r in a["rows"]]
        gen = a.get("generate")
        if gen:
            rows += [(gen["file"], i + 1, gen.get("who", ""), gen["source"].replace("{i}", str(i)), gen["target"].replace("{i}", str(i))) for i in range(gen["n"])]
        self.mem = Memory(rows)
        return {"rows": len(self.mem.rows), "common": sorted(self.mem.common)}

    @staticmethod
    def _skip(a):
        return tuple(a["skip"]) if a.get("skip") else None

    def op_memory_exact(self, a):
        return self.mem.exact_matches(a["source"], self._skip(a))

    def op_memory_similar(self, a):
        return self.mem.similar(a["source"], self._skip(a), a.get("limit", 5), a.get("min_ratio", 0.7))

    def op_memory_groups(self, a):
        return self.mem.groups(a.get("limit", 500), a.get("loose", False), a.get("by_speaker", False))

    def op_memory_tokens(self, a):
        from lexo import memory
        return {"tokens": [memory.tokens(t) for t in a["texts"]], "tags": [dict(memory.tags_of(t)) for t in a["texts"]]}

    @staticmethod
    def _snap(m):
        return None if m is None else {k: v for k, v in m.items() if k != "size"}          # the size of a zip depends on the compressor

    def op_snapshots_create(self, a):
        from lexo import snapshots
        return self._snap(snapshots.create(self.p(a["root"]), self.p(a["data"]), a.get("reason", "manual"), a.get("app", "")))

    def op_snapshots_list(self, a):
        from lexo import snapshots
        return [self._snap(m) for m in snapshots.list_snapshots(self.p(a["data"]))]

    def op_snapshots_prune(self, a):
        from lexo import snapshots
        return snapshots.prune(self.p(a["data"]), a["keep"])

    def op_snapshots_compare(self, a):
        from lexo import snapshots
        return snapshots.compare(self.p(a["root"]), self.p(a["data"]), a["name"])

    def op_snapshots_restore(self, a):
        from lexo import snapshots
        res = snapshots.restore(self.p(a["root"]), self.p(a["data"]), a["name"], a.get("files"), a.get("app", ""))
        return {"restored": res["restored"], "safety": self._snap(res["safety"])}

    def op_snapshots_delete(self, a):
        from lexo import snapshots
        snapshots.delete(self.p(a["data"]), a["name"])
        return True

    def op_snapshots_due(self, a):
        from lexo import snapshots
        return snapshots.due(self.p(a["data"]), a["interval_hours"])

    def op_snapshots_recent(self, a):
        from lexo import snapshots
        return snapshots.recent(self.p(a["data"]), a["minutes"])

    def op_snapshots_age(self, a):
        from lexo import snapshots
        return snapshots.last_age_hours(self.p(a["data"]))

    def op_snapshots_read(self, a):
        import zipfile
        with zipfile.ZipFile(self.p(a["data"]) / "snapshots" / a["name"]) as z:
            return sorted([i.filename, hashlib.sha1(z.read(i)).hexdigest()] for i in z.infolist())

    # -- the project and the writer of the .rpy files
    @staticmethod
    def _nat(s):
        return s.replace("/", os.sep)

    def op_project_check_dir(self, a):
        from lexo import project
        ok, m = project.check_project_dir(str(self.p(a["path"])))
        return {"ok": ok, "message": m or None}

    def op_project_find_tl(self, a):
        from lexo import project
        try:
            folder, note = project.find_translation_folder(self.p(a["path"]))
        except ValueError as e:
            return {"error": text_of(e)}
        return {"folder": str(folder), "note": note or None}

    def op_project_open(self, a):
        from lexo.project import Project
        self.project = Project(self.p(a["root"]), self.store, skip=a.get("skip", ()), symbols_as_done=a.get("symbols_as_done"))
        return True

    def op_project_refresh(self, a):
        self.report = self.project.refresh(force=a.get("force", False))
        return self.report.to_dict()

    def op_project_rows(self, a):
        from lexo.project import totals_of
        rows = self.project.rows()
        return {"rows": [list(r) for r in rows], "totals": list(totals_of(rows)), "files": sorted(self.project.files)}

    def op_project_proof(self, a):
        p, rel = self.project, self._nat(a["rel"])
        ents = p.entries(rel)
        return {"counts": list(p.counts(rel)), "proof_counts": list(p.proof_counts(rel)), "stale": p.proof_stale(rel),
                "states": [p.proof_state(rel, e) for e in ents], "translated": [p.is_translated(rel, e) for e in ents]}

    def op_project_path_of(self, a):
        try:
            return {"path": str(self.project.path_of(self._nat(a["rel"])))}
        except ValueError as e:
            return {"error": text_of(e)}

    def op_project_migrate(self, a):
        moved = self.project.migrate_metadata([(self._nat(o), self._nat(n)) for o, n in a["renames"]])
        return {"moved": moved, "state": self._dump_store()}

    def op_fixes_edit(self, a):
        from lexo import fixes
        return fixes.edit_translation(self.project, self.p(a["data"]), self._nat(a["rel"]), a["line"], a["text"])

    def op_fixes_edits(self, a):
        from lexo import fixes
        return fixes.apply_edits(self.project, self.p(a["data"]), [(self._nat(r), l, t) for r, l, t in a["edits"]], a["label"])

    def op_fixes_undo(self, a):
        from lexo import fixes
        return fixes.undo(self.project, self.p(a["data"]))

    def op_fixes_last(self, a):
        from lexo import fixes
        return fixes.last(self.p(a["data"]))

    def op_fixes_apply(self, a):
        from lexo import fixes
        plan = [{"file": self._nat(p["file"]), "line": p["line"], "before": p["before"], "after": p["after"], "skip": p.get("skip", "")} for p in a["plan"]]
        res = fixes.apply(self.project, self.p(a["data"]), plan)
        return {"fixed": res["fixed"], "skipped": [list(s) for s in res["skipped"]], "backups": res["backups"]}

    def op_fixes_check_literal(self, a):
        from lexo import fixes
        out = []
        for t in a["texts"]:
            try:
                fixes.check_literal(t)
                out.append(None)
            except fixes.FixError as e:
                out.append(text_of(e))
        return out

    # -- the public progress file and the bulk operations
    def op_export_build(self, a):
        from datetime import date, datetime
        from lexo import export
        self.progress = export.build_progress(self.project.rows(), self.store, self.hist, a.get("title", ""), now=datetime.fromisoformat(a["now"]), today=date.fromisoformat(a["today"]))
        return self.progress

    def op_export_validate(self, a):
        import copy
        from lexo import export
        data = copy.deepcopy(self.progress)
        for path, value in a.get("mutate", []):
            node = data
            for key in path[:-1]:
                node = node[key]
            node[path[-1]] = value
        if "replace" in a:
            data = a["replace"]
        return export.validate_progress(data)

    def op_export_render(self, a):
        from lexo import export
        return export.render(self.progress)

    def op_export_write(self, a):
        from lexo import export
        try:
            export.write_local(self.p(a["path"]), self.progress, self.p(a["backup_dir"]) if a.get("backup_dir") else None)
        except ValueError as e:
            return {"error": text_of(e)}
        return True

    def op_export_clean_target(self, a):
        from lexo import export
        out = []
        for rel in a["rels"]:
            try:
                out.append(export.clean_target(rel))
            except export.TargetError as e:
                out.append({"error": text_of(e)})
        return out

    def op_bulk_assign(self, a):
        from lexo import bulk
        res = bulk.apply_assignments(self.store, [{"name": self._nat(i["name"]), "value": i.get("value", ""), "overwrite": i.get("overwrite", False)} for i in a["items"]])
        return {"applied": res["applied"], "skipped": [list(s) for s in res["skipped"]], "state": self._dump_store()}

    def op_bulk_candidates(self, a):
        from lexo import bulk
        try:
            return bulk.approval_candidates(self.project, self._nat(a["rel"]), a["line"], a.get("scope", "project"), a.get("same_speaker", True))
        except ValueError as e:
            return {"error": text_of(e)}

    def op_bulk_approve(self, a):
        from lexo import bulk
        res = bulk.apply_approvals(self.project, [{"file": self._nat(s["file"]), "key": s["key"]} for s in a["selections"]], a.get("on", True), a.get("label"))
        return {"changed": res["changed"], "skipped": [list(s) for s in res["skipped"]], "state": self._dump_store()}

    def op_bulk_rules(self, a):
        from lexo import bulk
        rels = [self._nat(r) for r in a["rels"]] if a.get("rels") else None
        return bulk.rule_candidates(self.project, self.store.glossary, a["rules"], rels)


# =========================================================================================================================== everything
def collect():
    """{path under expected/: object} as Core answers it today."""
    out = {"rpy.json": run_rpy(), "rpa.json": run_rpa(), "layout.json": run_layout(), "menus.json": run_menus(), "validator.json": run_validator(),
           "spelling.json": run_spelling(), "textedit.json": run_textedit(), "fontmetrics.json": run_fontmetrics()}
    for name in scenario_names():
        out[f"scenarios/{name}.json"] = run_scenario(GOLDEN / "scenarios" / f"{name}.json")
    for name in data_names():
        out[f"data/{name}.json"] = run_data_scenario(GOLDEN / "data" / f"{name}.json")
    return out


def generate():
    shutil.rmtree(EXPECTED, ignore_errors=True)
    got = collect()
    for rel, obj in got.items():
        dump(EXPECTED / rel, obj)
    return sorted(got)


if __name__ == "__main__":
    print("\n".join(generate()))
