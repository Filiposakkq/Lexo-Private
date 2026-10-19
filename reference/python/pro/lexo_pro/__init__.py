"""Lexo Pro pack (private, delivered only to licensed users; never part of the public repository).

Core finds this package by its name (`lexo_pro`, see lexo/extensions.py) and calls `provider()`. Everything here depends on Core (`lexo`), never the other way round.
The pack decides on its own what it offers and with which limits (`offers`): the place for the licence check and for the demo mode of an unlicensed pack.
Today it offers everything it contains, because the licence check does not exist yet (roadmap step 6).
"""
import json
from pathlib import Path

from lexo import layout, messages
from lexo.extensions import API, ExtensionError

from . import categorize, fix_safe, licence, mt_batch, publish, regex_replace

__version__ = "0.1.0"


class Provider:
    api = API
    version = __version__

    def offers(self):
        return {f: o for f in ("publish.web", "bulk.categorize", "replace.regex", "mt.batch", "fixes.safe") for o in [licence.offer(f)] if o is not None}

    def ui_strings(self, code):
        """The Czech (or other) texts of the messages of this pack, from ui/<code>.json."""
        path = Path(__file__).resolve().parents[4] / "pro" / "rust" / "lexo-pro" / "ui" / f"{code}.json"   # the texts belong to the Rust pack (pro/rust/lexo-pro/ui)
        try:
            return json.loads(path.read_text(encoding="utf-8")).get("strings", {})
        except (OSError, ValueError):
            return {}

    def call(self, feature, method, payload):
        if feature == "publish.web":
            return self._publish(method, payload)
        if feature == "bulk.categorize" and method == "suggest":
            return self._categorize(payload)
        if feature == "fixes.safe" and method in ("plan", "chain"):
            return messages.wire(getattr(fix_safe, method)(payload))
        if feature == "mt.batch" and method in ("targets", "translate"):
            return messages.wire(getattr(mt_batch, method)(payload))
        if feature == "replace.regex" and method == "substitute":
            return messages.wire(regex_replace.substitute(payload))
        raise ExtensionError(f"The Pro pack has no feature {feature}.")

    def _categorize(self, p):
        names, cut = licence.cap("bulk.categorize", p["names"])
        lay = layout.parse(p["layout"])
        items = categorize.suggest(names, p["current"], known_assign=p["known"], lay=lay)
        return messages.wire({**cut, "items": items})

    def _publish(self, method, p):
        try:
            if method == "plan":
                out = publish.plan(p["web"], p["data"], p["text"])
            elif method == "publish":
                out = publish.publish(p["web"], p["data"], p["text"], p["token"], p["branch"])
            else:
                raise ExtensionError(f"publish.web has no method {method}.")
        except publish.PublishError as e:
            raise ExtensionError(*e.args) from e              # the message (with its template) goes on to the user
        return messages.wire(out)                              # plain JSON: messages travel as {"$m": template, "p": values}


def provider():
    return Provider()
