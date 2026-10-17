"""The interface contract of Core: a fixture project, the shapes of the answers, and the generated documents.

    py core/tests/contract_tool.py            rewrite core/contract/api.json, core/contract/shapes.json and docs/API.md

`api.json` is `lexo.api.describe()` (what every operation takes), `shapes.json` is what every operation answered on the fixture project (names and types of the
keys, no values), `docs/API.md` is the readable form. `test_contract.py` keeps them in step with the code: changing an operation without regenerating them fails
a test, which is the point: the interface changes on purpose, in one visible place. The same files are what the Rust port of Core is checked against.
"""
import json
import os
import shutil
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path[:0] = [str(HERE.parent), str(HERE)]

from lexo import api                      # noqa: E402
from lexo.messages import render, wire    # noqa: E402

CONTRACT_DIR = ROOT / "core" / "contract"
DOC = ROOT / "docs" / "API.md"
DYNAMIC_AFTER = 25          # an object with more keys than this is a map of unknown keys (texts, names), not a record
MAX_DEPTH = 5

GAME_SCRIPT = '''label start:
    scene room
    show anna smile at left
    mc "Hello there, friend."
    menu:
        "Invite her":
            mc "Coffee?"
        "Leave":
            pass
    mc "Bye."
'''
TRANSLATION = '''# game/script.rpy:4
translate czech a1:

    # mc "Hello there, friend."
    mc "Ahoj tam, příteli."

# game/script.rpy:7
translate czech a2:

    # mc "Coffee?"
    mc "Coffee?"

# game/script.rpy:10
translate czech a3:

    # mc "Bye."
    de "Nashle."

translate czech strings:

    # game/script.rpy:5
    old "Invite her"
    new "Pozvat ji"

    # game/script.rpy:5
    old "Leave"
    new "Leave"
'''
# the lines of the translation file the examples of api.py point at: 5 = the first dialogue line, 23 = the choice "Invite her"


def build_fixture(base):
    """<base>/game with a script and <base>/game/tl/czech with its translation. Returns the translation folder."""
    game = Path(base) / "game"
    tl = game / "tl" / "czech"
    tl.mkdir(parents=True)
    (game / "script.rpy").write_text(GAME_SCRIPT, encoding="utf-8", newline="\n")
    (tl / "script.rpy").write_text(TRANSLATION, encoding="utf-8", newline="\n")
    return tl


def open_fixture():
    """(app, cleanup): an App on a fresh fixture project, with its own settings folder."""
    from lexo.app import App
    from lexo.config import Config
    tmp = Path(tempfile.mkdtemp(prefix="lexo-contract-"))
    old_cfg = os.environ.get("LEXO_CONFIG_DIR")
    os.environ["LEXO_CONFIG_DIR"] = str(tmp / "cfg")
    app = App(Config().load())
    app.open_project(build_fixture(tmp / "g"))
    app.glossary_set([{"code": "friend", "name": "pritel", "type": "term"}])          # the example of glossary/occurrences

    def cleanup():
        try:
            app.close()
        finally:
            if old_cfg is None:
                os.environ.pop("LEXO_CONFIG_DIR", None)
            else:
                os.environ["LEXO_CONFIG_DIR"] = old_cfg
            shutil.rmtree(tmp, ignore_errors=True)
    return app, cleanup


# ------------------------------------------------------------------------------------------------------------------------------ shapes
def shape(value, depth=0):
    """Names and types of what an answer holds (no values): object -> keys, array -> the shape of the first item, the rest -> its JSON type."""
    if value is None:
        return {"type": "null"}
    if isinstance(value, bool):
        return {"type": "boolean"}
    if isinstance(value, (int, float)):
        return {"type": "number"}
    if isinstance(value, str):
        return {"type": "string"}
    if isinstance(value, dict):
        if depth >= MAX_DEPTH:
            return {"type": "object"}
        if len(value) > DYNAMIC_AFTER:
            return {"type": "object", "dynamic": True}
        return {"type": "object", "keys": {k: shape(value[k], depth + 1) for k in sorted(value)}}
    if isinstance(value, (list, tuple)):
        if depth >= MAX_DEPTH:
            return {"type": "array"}
        return {"type": "array", "items": shape(value[0], depth + 1) if value else None}
    return {"type": "string"}


def compatible(recorded, live, path="$"):
    """Problems where `live` no longer fits `recorded`: a key that vanished, a type that changed. New keys are fine (re-record to note them)."""
    if recorded is None or live is None:
        return []
    rt, lt = recorded["type"], live["type"]
    if rt == "null" or lt == "null":
        return []                                   # an optional value that was empty (or is empty now)
    if rt != lt:
        return [f"{path}: was {rt}, is {lt}"]
    out = []
    if rt == "object" and "keys" in recorded and "keys" in live:
        for k, sub in recorded["keys"].items():
            if k not in live["keys"]:
                out.append(f"{path}.{k}: missing")
            else:
                out += compatible(sub, live["keys"][k], f"{path}.{k}")
    if rt == "array":
        out += compatible(recorded.get("items"), live.get("items"), path + "[]")
    return out


def plain(result):
    """The answer as the client sees it, with messages as their English text."""
    return render(json.loads(json.dumps(wire(result), default=str)))


# ------------------------------------------------------------------------------------------------------------------------------ running every operation
def run_examples(app):
    """{name: answer (plain)} for every operation that has an example, reads first, then the writes (in the order of the table)."""
    out, order = {}, sorted(api.OPS.values(), key=lambda o: (o.effect != "read", o.name))
    for op in order:
        if op.example is None or op.effect == "external" or op.kind == "binary":
            continue
        out[op.name] = plain(api.call(app, op.name, op.example))
    return out


def record(app):
    return {name: shape(ans) for name, ans in run_examples(app).items()}


# ------------------------------------------------------------------------------------------------------------------------------ documents
def render_doc():
    ops = api.describe()
    lines = ["# The interface of Core (generated)", "",
             "Generated from `core/lexo/api.py` by `py core/tests/contract_tool.py`; do not edit by hand (a test fails when it is out of date).", "",
             "Every operation is a JSON call: parameters in, a dict with `\"ok\": true` out. Over HTTP (Free) a GET takes its parameters in the query string, a POST in the JSON body;",
             "a script or the Rust port calls the operation by name. Errors come back as `{\"ok\": false, \"error\": ...}`. Messages in answers are `{\"$m\": template, \"p\": values}`",
             "(see `docs/UI-TEXTS.md`). Effects: **read** changes nothing, **write** changes the project or its data folder, **external** also reaches outside",
             "(a program, a window, the network). `*` marks a required parameter. The recorded shapes of the answers are in `core/contract/shapes.json`.", "",
             f"{len(ops)} operations.", "",
             "| Operation | Method / path | Effect | Parameters | What it does |", "|---|---|---|---|---|"]
    for o in ops:
        params = ", ".join(f"`{p['name']}`{'*' if p['required'] else ''} ({p['type']}{'' if p['default'] in ('', 0, False, [], None) else ', default ' + json.dumps(p['default'])})" for p in o["params"]) or "-"
        extra = " (needs no open project)" if not o["needs_project"] else ""
        kind = " **binary**" if o["kind"] == "binary" else ""
        lines.append(f"| `{o['name']}` | {o['method']} `/api/{o['path']}` | {o['effect']}{kind} | {params} | {o['doc']}{extra} |")
    return "\n".join(lines) + "\n"


def write_all():
    CONTRACT_DIR.mkdir(parents=True, exist_ok=True)
    (CONTRACT_DIR / "api.json").write_text(json.dumps(api.describe(), ensure_ascii=False, indent=1) + "\n", encoding="utf-8", newline="\n")
    app, cleanup = open_fixture()
    try:
        shapes = record(app)
    finally:
        cleanup()
    (CONTRACT_DIR / "shapes.json").write_text(json.dumps(shapes, ensure_ascii=False, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    DOC.write_text(render_doc(), encoding="utf-8", newline="\n")
    return len(api.OPS), len(shapes)


if __name__ == "__main__":
    n, m = write_all()
    print(f"{n} operations, {m} recorded shapes")
