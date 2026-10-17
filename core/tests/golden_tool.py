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


# =========================================================================================================================== everything
def collect():
    """{path under expected/: object} as Core answers it today."""
    out = {"rpy.json": run_rpy(), "rpa.json": run_rpa(), "layout.json": run_layout(), "menus.json": run_menus(), "validator.json": run_validator()}
    for name in scenario_names():
        out[f"scenarios/{name}.json"] = run_scenario(GOLDEN / "scenarios" / f"{name}.json")
    return out


def generate():
    shutil.rmtree(EXPECTED, ignore_errors=True)
    got = collect()
    for rel, obj in got.items():
        dump(EXPECTED / rel, obj)
    return sorted(got)


if __name__ == "__main__":
    print("\n".join(generate()))
