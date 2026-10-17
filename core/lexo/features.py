"""Feature registry: which features exist, which tier they belong to, and what the current licence allows.

The application never decides "is this Pro?" on its own. At the few places where a Pro feature is used it asks
`Entitlements.decide(feature)` and gets an answer:

    available   the feature may be used
    limit       None = unlimited, N = at most N items per action (the free "taste": it works, on a small amount)
    total_hint  not here: the application knows how many items there are and tells the user ("37 lines, Free does 20")

The catalog below is the single list of gated features (see docs/FEATURES.md). Everything not in the catalog is free.
Where the entitlements come from is the only thing that changes when licensing exists: today `current()` is a
development default (everything on), later the Pro loader provides a licence-based `Entitlements`.
"""
import os
from dataclasses import dataclass


@dataclass(frozen=True)
class Feature:
    id: str
    title: str
    free_limit: object = "locked"       # "locked" or the number of items per action Free may use (only for a feature that is not extracted yet)
    extracted: bool = False             # True: the code is in the Pro pack and the extension host decides (extensions.py); False: still in Core, gated by `Entitlements`


CATALOG = {f.id: f for f in (
    Feature("mt.batch", "Batch machine translation", 20, extracted=True),
    Feature("fixes.safe", "Safe bulk fixes with preview", 20),
    Feature("replace.regex", "Regex find & replace", "locked", extracted=True),
    Feature("bulk.categorize", "Bulk categorisation suggestions", 20, extracted=True),
    Feature("publish.web", "Publishing progress to a website", "locked", extracted=True),
)}


@dataclass(frozen=True)
class Decision:
    feature: str
    available: bool
    limit: object = None                # None = unlimited


class Entitlements:
    """What the user may use. `pro=True` unlocks every catalog feature without limits."""

    def __init__(self, pro=False, source=""):
        self.pro = pro
        self.source = source

    def decide(self, feature):
        spec = CATALOG.get(feature)
        if spec is None:
            return Decision(feature, True)                    # not gated
        if self.pro:
            return Decision(feature, True)
        if spec.free_limit == "locked":
            return Decision(feature, False)
        return Decision(feature, True, int(spec.free_limit))

    def status(self):
        """For the UI: every gated feature with its decision (the features that are not extracted yet; see `status_of`)."""
        return {fid: {"title": spec.title, "available": d.available, "limit": d.limit, "pro": self.pro}
                for fid, spec in CATALOG.items() for d in [self.decide(fid)]}


FREE = Entitlements(False, "free")
PRO = Entitlements(True, "pro")


def default():
    """DEVELOPMENT default: everything on. It becomes FREE when the licence loader exists (roadmap step 6).
    LEXO_TIER=free lets a developer try the free behaviour."""
    return FREE if os.environ.get("LEXO_TIER", "").lower() == "free" else Entitlements(True, "dev")


def decide(feature, entitlements, host):
    """The decision for one feature: an extracted feature is the business of the extension host (a Pro pack must offer it), the others of `entitlements`."""
    spec = CATALOG.get(feature)
    if spec is not None and spec.extracted:
        return host.decide(feature)
    return entitlements.decide(feature)


def status_of(entitlements, host):
    """For the UI: every gated feature, whether it is available and with which limit, and whether a Pro pack is there at all."""
    out = {}
    for fid, spec in CATALOG.items():
        d = decide(fid, entitlements, host)
        out[fid] = {"title": spec.title, "available": d.available, "limit": d.limit, "pro": host.present if spec.extracted else entitlements.pro, "extracted": spec.extracted}
    return out

