"""Feature catalog: which features are gated (Pro), and what the user is told about them.

The application never decides "is this Pro?" on its own, and no flag in Core or Free can switch a Pro feature on. At the few places where a Pro feature is used it asks the
extension host (extensions.py) `decide(feature)` and gets an answer:

    available   the Pro pack offers the feature right now (without the pack, nothing is available)
    limit       None = unlimited, N = at most N items per action (the "taste" of the demo pack; the pack enforces it itself)

The catalog below is the single list of gated features (see docs/FEATURES.md). Everything not in the catalog is free. Every feature of it lives in the Pro pack (reference/python/pro/lexo_pro).
"""
from dataclasses import dataclass


@dataclass(frozen=True)
class Feature:
    id: str
    title: str


CATALOG = {f.id: f for f in (
    Feature("mt.batch", "Batch machine translation"),
    Feature("fixes.safe", "Safe bulk fixes with preview"),
    Feature("replace.regex", "Regex find & replace"),
    Feature("bulk.categorize", "Bulk categorisation suggestions"),
    Feature("publish.web", "Publishing progress to a website"),
)}


@dataclass(frozen=True)
class Decision:
    feature: str
    available: bool
    limit: object = None                # None = unlimited


def decide(feature, host):
    """The decision for one feature: a feature outside the catalog is free, a gated one is what the Pro pack offers (nothing without the pack)."""
    if feature not in CATALOG:
        return Decision(feature, True)
    return host.decide(feature)


def status_of(host):
    """For the UI: every gated feature, whether it is available and with which limit, and whether a Pro pack is there at all."""
    out = {}
    for fid, spec in CATALOG.items():
        d = decide(fid, host)
        out[fid] = {"title": spec.title, "available": d.available, "limit": d.limit, "pro": host.present}
    return out
