"""Batch machine translation (the Pro feature mt.batch): which lines a batch takes, and the translating of many lines.

Core chooses the candidate lines, gathers each line's context and glossary terms and later checks and writes the suggestions; this module cuts the list to the limit
of the licence and does the translating (it uses Core's single-line `mt.translate`, which stays free). Plain data in, plain data out.
"""
from lexo import mt
from lexo.messages import text_of

from . import licence

HTTP = None          # tests put a fake transport here; None = the real network


def targets(p):
    """{items: [{file, line}], total, chars, limited?}: the candidates {file, line, chars} cut to the limit."""
    items, cut = licence.cap("mt.batch", p["items"])
    return {**cut, "items": [{"file": i["file"], "line": i["line"]} for i in items], "total": len(p["items"]), "chars": sum(int(i["chars"]) for i in items)}


def translate(p):
    """Suggestions for the requests [{id, source, context, terms}] with the settings `cfg`. A failed line carries its own error; a rejected key, missing consent
    or a broken connection stops the batch (the same message for every line would be useless)."""
    out = []
    for r in p["requests"]:
        try:
            res = mt.translate(p["cfg"], r["source"], r["terms"], r["context"], http=HTTP)
        except mt.MTError as e:
            err = text_of(e)
            out.append({"id": r["id"], "error": err})
            if str(err) != str(mt.NO_TEXT):
                return {"results": out, "stopped": err}
            continue
        out.append({"id": r["id"], "suggestion": res["text"], "warnings": res["warnings"], "provider": res["provider"]})
    return {"results": out, "stopped": ""}
