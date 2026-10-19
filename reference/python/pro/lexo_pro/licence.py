"""What the pack offers, and with which limits. The licence check will live here (roadmap step 6).

Two kinds of Pro features (docs/product/PRO-DESIGN.md): "taste" features work in demo mode with a limit, everything else is offered only with a licence.
Until the licence file exists the pack is "licensed" in a development checkout; `LEXO_PRO_DEMO=1` shows the demo (unlicensed) behaviour.
"""
import os

DEMO_LIMITS = {"bulk.categorize": 5, "mt.batch": 20, "fixes.safe": 20}           # the taste features: items per action in demo mode (more are added as features move into the pack)


def licensed():
    return os.environ.get("LEXO_PRO_DEMO", "") != "1"


def offer(feature):
    """{"limit": None | N} when the pack offers `feature` now, otherwise None."""
    if licensed():
        return {"limit": None}
    if feature in DEMO_LIMITS:
        return {"limit": DEMO_LIMITS[feature]}
    return None


def cap(feature, seq):
    """Cut `seq` to the limit of `feature`. Returns (items, info); info says what was cut, for the UI."""
    seq = list(seq)
    limit = (offer(feature) or {}).get("limit")
    if limit is None or len(seq) <= limit:
        return seq, {}
    return seq[:limit], {"limited": {"feature": feature, "limit": limit, "total": len(seq)}}
