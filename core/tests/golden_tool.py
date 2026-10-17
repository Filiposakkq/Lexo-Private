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
    stack = [mock.patch(f"lexo.{m}.datetime", FakeDT) for m in ("util", "events", "texthistory", "snapshots")]
    stack += [mock.patch("lexo.util.time", fake_time), mock.patch("lexo.lock.time", fake_time)]
    return stack


def _file_records(root, skip=()):
    files = {}
    for f in sorted(root.rglob("*")):
        if f.is_file() and f.name not in skip:
            b = f.read_bytes()
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
        runner = DataRunner(root, tmp / "cfg", clock)
        steps = []
        with ExitStack() as stack:
            for p in _patches(clock):
                stack.enter_context(p)
            for st in spec["steps"]:
                entry = {"op": st["op"], "params": {k: v for k, v in st.items() if k != "op"}}
                try:
                    entry["answer"] = plain(runner.run(st["op"], st))
                except (AppError, WriteFailed) as e:
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
