"""The golden cases of the Pro pack: payloads and the answers the Python pack gives, which the Rust pack (pro/rust) is checked against.

    py pro/tests/golden_pro.py            rewrite pro/golden/pack.json (look at the diff before committing it)
    py pro/tests/golden_pro.py --check    fail when the file is not what the Python pack gives now

A case is `{id, feature, method, payload, env?, expect}` where `expect` is `{"answer": ...}` or `{"error": ...}` (messages as they travel: `{"$m": ...}`) and `env` sets environment variables for the
call (the demo mode). A case whose id starts with `bad-` is compared only as "it is an error": the wording comes from the regular expression engine and differs between the two languages.
"""
import json
import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path[:0] = [str(ROOT / "core"), str(ROOT / "core" / "tests"), str(ROOT / "pro")]

import lexo_pro                                   # noqa: E402
from lexo import layout                           # noqa: E402
from lexo.extensions import ExtensionError        # noqa: E402
from lexo.messages import wire                    # noqa: E402

OUT = ROOT / "pro" / "golden" / "pack.json"


def fake_http(url, headers, data):
    """The network of the machine translation cases: "CZ " + the text; the key "bad" is refused. The Rust test has the same function."""
    from urllib.parse import parse_qs
    body = data.decode("utf-8")
    key = headers.get("Authorization", "")
    if "bad" in key or '"api_key": "bad"' in body:
        return 403, "forbidden"
    if "deepl" in url:
        text = parse_qs(body)["text"][0]
        return 200, json.dumps({"translations": [{"text": "CZ " + text}]})
    if url.endswith("/chat/completions"):
        user = json.loads(body)["messages"][-1]["content"]
        return 200, json.dumps({"choices": [{"message": {"content": "CZ " + user.split("Translate:\n", 1)[1]}}]})
    return 200, json.dumps({"translatedText": "CZ " + json.loads(body)["q"]})

EIGHT = [{"n": 1, "status": "active", "groups": [1, 2, 3, 4]}, {"n": 2, "status": "active", "groups": [5, 6, 7, 8]}]
LAYOUTS = {
    "eight": {"version": 1, "language": "cs", "sections": EIGHT},
    "one": {"version": 1, "sections": [{"n": 1, "status": "active", "groups": [1, 2, 3]}], "labels": {"group": "Chapter"}},
    "folders": {"version": 1, "folders": ["beach", "home", "work"], "sections": [{"n": 1, "status": "active", "groups": [1, 2, 3]}], "labels": {"group": "Folder"}},
    "series": {"version": 1, "series": "script", "sections": [{"n": 1, "status": "active", "groups": [1, 2, 3, 4]}]},
    "none": {"version": 1},
}

NAMES = ["scripts/freeroam/episode3/ep3_x.rpy", "season2/scripts/madame_ep7.rpy", "scripts/episode4/bella.rpy", "step5_map.rpy", "deep2.rpy", "season2/scripts/update6.rpy", "ep2_ep3_mix.rpy",
         "season2/ep3_ep6.rpy", "season2/scripts/ep3_oops.rpy", "ep12_future.rpy", "ep4_x.rpy", "options.rpy", "screens.rpy", "gui.rpy", "script.rpy", "script2.rpy", "script3.rpy",
         "scripts/home/a.rpy", "scripts/home/b.rpy", "beach/x.rpy", "work/y.rpy", "chapter2.rpy", "season1/update2.rpy", "unknown_thing.rpy", "sub\\ep5_win.rpy", "Ep1.RPY",
         "season3/ep2.rpy", "common.rpy", "title_screens.rpy"]

TEXTS = ["Petrovi a Honzovi", "nothing", "Hello", "a{i}b", "a{i}[name]b i", "{i}", "Ahoj  tam, jak se máš?", "Příliš žluťoučký kůň", "x\\ny", "AbC abc ABC", "", "tea tee ten"]


def categorize_cases():
    for lay_name, lay in LAYOUTS.items():
        for known_name, current, known in (("empty", {}, {}), ("assigned", {"ep4_x.rpy": "ep:2", "scripts/home/a.rpy": "ep:1"}, None),
                                           ("folder", {}, {"scripts/home/a.rpy": "ep:1", "scripts/mixed/x.rpy": "ep:1", "scripts/mixed/y.rpy": "ep:2", "scripts/mixed/z.rpy": "ep:2"})):
            yield f"categorize-{lay_name}-{known_name}", "bulk.categorize", "suggest", {"names": NAMES, "current": current, "known": current if known is None else known, "layout": lay}


def regex_cases():
    pats = [(r"(\w+)ovi", r"\1u"), ("hello", "x"), (r"a.*b", "X"), ("i", "X"), (r"(?P<w>t)e", r"\g<w>E"), (r"\bte(a|e|n)\b", r"[\1]"), (r"(?<=a) ", "_"), (r"(\w)\1", r"<\1\1>"), ("[aeiouyáéíóúůý]", "*"),
            (r"\s{2,}", " "), ("a|b", r"\g<0>\g<0>"), (r"(x)?a", r"[\1]"), ("l+", r"\n"), ("x", r"\0"), (r"\w+", r"\\"), (r"(?i)ahoj", "Čau")]
    for i, (p, r) in enumerate(pats):
        for case in (False, True):
            for protect in (True, False):
                yield f"regex-{i}-case{int(case)}-prot{int(protect)}", "replace.regex", "substitute", {"pattern": p, "replace": r, "case": case, "protect": protect, "texts": TEXTS}
    for i, (p, r) in enumerate([("(", "x"), ("a", r"\9"), ("", "x"), ("a", r"\q"), ("a" * 301, "x"), ("a", "x" * 2001), ("a", r"\g<nope>")]):
        yield f"bad-regex-{i}", "replace.regex", "substitute", {"pattern": p, "replace": r, "case": False, "protect": True, "texts": TEXTS}


def fix_cases():
    speaker = [
        {"file": "a.rpy", "line": 10, "src_who": "mc happy", "src_suffix": "", "cur_raw": '    mc sad "Ahoj."', "src_raw": '    mc happy "Hello."'},
        {"file": "a.rpy", "line": 14, "src_who": "mc", "src_suffix": "with dissolve", "cur_raw": '    de "Ahoj."', "src_raw": '    mc "Hello." with dissolve'},
        {"file": "b.rpy", "line": 3, "src_who": "", "src_suffix": "", "cur_raw": '    "bez uvozovek"', "src_raw": '    "bez uvozovek"'},
        {"file": "b.rpy", "line": 5, "src_who": "mc", "src_suffix": "", "cur_raw": "    mc no quotes here", "src_raw": '    mc "x"'},
        {"file": "B.rpy", "line": 1, "src_who": "mc", "src_suffix": "", "cur_raw": '\tmc "Tab" ', "src_raw": '\tmc "Tab"'},
    ]
    text = [
        {"file": "a.rpy", "line": 20, "source": "Weird.", "target": "Já  prijdu zítra.", "issues": [{"word": "prijdu", "better": "přijdu", "typo_kind": "diacritics"}]},
        {"file": "a.rpy", "line": 24, "source": "Two  spaces", "target": "Dvě  mezery", "issues": []},
        {"file": "a.rpy", "line": 28, "source": "Hello {i}x{/i}", "target": "Ahoj  {i}x  y{/i}  [name]", "issues": []},
        {"file": "b.rpy", "line": 9, "source": "Fine.", "target": "Dobře.", "issues": [{"word": "Dobre", "better": "Dobře", "typo_kind": "diacritics"}, {"word": "tset", "better": "test", "typo_kind": "swap"}]},
        {"file": "b.rpy", "line": 11, "source": "A", "target": "Prijdu a prijdu znovu", "issues": [{"word": "prijdu", "better": "přijdu", "typo_kind": "diacritics"}, {"word": "", "better": "x"}]},
        {"file": "c.rpy", "line": 1, "source": "Short", "target": "Kratke", "issues": [{"word": "Kratke", "better": "Krátké", "typo_kind": "diacritics"}]},
    ]
    for kinds in (["speaker"], ["spaces"], ["typo"], ["speaker", "spaces", "typo"], [], ["nonsense"], ["spaces", "spaces"]):
        yield f"fix-plan-{'-'.join(kinds) or 'default'}", "fixes.safe", "plan", {"kinds": kinds, "speaker": speaker, "text": text}
    many = [{"file": f"f{i:02d}.rpy", "line": i, "source": "x", "target": "a  b", "issues": []} for i in range(25)]
    yield "fix-plan-many", "fixes.safe", "plan", {"kinds": ["spaces"], "speaker": [], "text": many}
    yield "fix-plan-many-demo", "fixes.safe", "plan", {"kinds": ["spaces"], "speaker": [], "text": many}, {"LEXO_PRO_DEMO": "1"}
    items = [{"file": c["file"], "line": c["line"], "source": c["source"], "target": c["target"], "kinds": ks, "issues": c["issues"]}
             for c in text for ks in (["spaces"], ["typo"], ["spaces", "typo"])]
    yield "fix-chain", "fixes.safe", "chain", {"items": items}


def mt_cases():
    cands = [{"file": f"f{i % 3}.rpy", "line": i, "chars": 10 + i} for i in range(30)]
    yield "mt-targets", "mt.batch", "targets", {"items": cands}
    yield "mt-targets-demo", "mt.batch", "targets", {"items": cands}, {"LEXO_PRO_DEMO": "1"}
    yield "mt-targets-empty", "mt.batch", "targets", {"items": []}
    base = {"id": 0, "source": "Hello {i}friend{/i}", "context": ["Before."], "terms": {"friend": "přítel"}}
    for name, cfg in (("no-provider", {}), ("no-consent", {"provider": "deepl", "api_key": "k", "target_lang": "CS"}), ("no-key", {"provider": "deepl", "consent": True, "target_lang": "CS"}),
                      ("no-target", {"provider": "openai", "consent": True, "api_key": "k"}), ("bad-url", {"provider": "deepl", "consent": True, "api_key": "k", "base_url": "ftp://x", "target_lang": "CS"})):
        yield f"mt-translate-{name}", "mt.batch", "translate", {"cfg": cfg, "requests": [base, {**base, "id": 1}]}
    cfg = {"provider": "deepl", "consent": True, "api_key": "k", "target_lang": "CS"}
    for prov, extra in (("deepl", {}), ("openai", {"model": "m"}), ("libretranslate", {"base_url": "http://localhost:5000"})):
        yield f"mt-translate-ok-{prov}", "mt.batch", "translate", {"cfg": {**cfg, "provider": prov, **extra}, "requests": [base, {**base, "id": 1, "source": "[name] said {b}x{/b}", "context": [], "terms": {}}]}
    yield "mt-translate-refused-key", "mt.batch", "translate", {"cfg": {**cfg, "api_key": "bad"}, "requests": [base, {**base, "id": 1}]}
    yield "mt-translate-empty-text-first", "mt.batch", "translate", {"cfg": cfg, "requests": [{**base, "source": "  "}, {**base, "id": 1}]}
    yield "mt-translate-none", "mt.batch", "translate", {"cfg": cfg, "requests": []}


def all_cases():
    out = []
    for gen in (categorize_cases, regex_cases, fix_cases, mt_cases):
        for case in gen():
            cid, feature, method, payload = case[:4]
            env = case[4] if len(case) > 4 else None
            out.append((cid, feature, method, payload, env))
    return out


def run(feature, method, payload, env):
    from lexo_pro import mt_batch
    mt_batch.HTTP = fake_http
    old = {k: os.environ.get(k) for k in (env or {})}
    os.environ.update(env or {})
    try:
        provider = lexo_pro.provider()
        if feature == "bulk.categorize":
            layout.use(layout.parse(payload["layout"]))                     # the app has the layout of the project in use while the pack suggests
        try:
            return {"answer": json.loads(json.dumps(provider.call(feature, method, json.loads(json.dumps(payload)))))}
        except ExtensionError as e:
            first = e.args[0] if e.args else ""
            return {"error": json.loads(json.dumps(wire(first)))}
    finally:
        for k, v in old.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v


def build():
    cases = []
    for cid, feature, method, payload, env in all_cases():
        case = {"id": cid, "feature": feature, "method": method, "payload": payload, "expect": run(feature, method, payload, env)}
        if env:
            case["env"] = env
        cases.append(case)
    # the offers in the licensed and in the demo mode
    offers = {}
    for name, env in (("licensed", {}), ("demo", {"LEXO_PRO_DEMO": "1"})):
        old = os.environ.get("LEXO_PRO_DEMO")
        os.environ.pop("LEXO_PRO_DEMO", None)
        os.environ.update(env)
        offers[name] = lexo_pro.provider().offers()
        os.environ.pop("LEXO_PRO_DEMO", None)
        if old is not None:
            os.environ["LEXO_PRO_DEMO"] = old
    return {"offers": offers, "ui_cs": lexo_pro.provider().ui_strings("cs"), "ui_xx": lexo_pro.provider().ui_strings("xx"), "cases": cases}


def main():
    text = json.dumps(build(), ensure_ascii=False, indent=1) + "\n"
    if "--check" in sys.argv:
        same = OUT.is_file() and OUT.read_text(encoding="utf-8") == text
        print("pack.json is up to date" if same else "pack.json is out of date: py pro/tests/golden_pro.py")
        sys.exit(0 if same else 1)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8", newline="\n")
    print(OUT, len(json.loads(text)["cases"]), "cases")


if __name__ == "__main__":
    main()
