"""Extension host: how a separately delivered Pro pack plugs into Core, and how a Pro feature is called.

Why it exists: a Pro feature must not be in the public code at all, and must not be switched on by editing a flag in it. So Core and Free hold only

  * the catalog of gated features (features.py: id, title, what the user is told when it is missing),
  * this host, which asks "is there a provider for this feature, and what does it offer?",
  * the calls themselves: `host.call("publish.web", "plan", {...})`.

The implementation (and the licence check, and the limits of the demo mode) lives in the Pro pack, a package called `lexo_pro` that is installed
next to the application only for people who have a licence. Without it the host answers "not available" and nothing else can change that.

The contract between the host and a provider is plain JSON, on purpose: a call is a feature id, a method name and a dict that can be written
as JSON, the answer is such a dict, an error is an `ExtensionError` (a message for the user). The host sends every payload through JSON and back, so a
provider can never receive or return a live Python object. That makes the transport replaceable: today the provider is a Python module in the same
process, later it can be a separate program (a sidecar, speaking the same JSON on its standard input and output) without any caller changing.
A Pro feature computes and returns data (a plan, proposals, translations); the one writer of `.rpy` files stays in Core.

A provider is an object with:
    api          int, the version of this contract (`API`); a provider with another one is refused, not half used
    version      str, the version of the pack (shown to the user)
    ui_strings(code) -> {English template: translation}   optional: the interface texts of the messages the pack writes
    offers()     {feature id: {"limit": None | N}}   what it can do right now (a demo or unlicensed pack offers less; the pack enforces its own limits)
    call(feature, method, payload) -> dict
"""
import importlib
import json

from .features import Decision

API = 1
PACK_MODULE = "lexo_pro"


class ExtensionError(Exception):
    """A provider refused or failed; the message is meant for the user."""


def _json_roundtrip(value, what):
    try:
        return json.loads(json.dumps(value))
    except (TypeError, ValueError) as e:
        raise ExtensionError(f"{what} is not plain JSON: {e}") from e


class ExtensionHost:
    def __init__(self, trust=None):
        """`trust(module)` may veto a pack before it is used (the place for the signature check of the pack; None = no check, for development)."""
        self._providers = []
        self._trust = trust
        self.problems = []                 # why a pack was not used (shown in the settings, never fatal)

    # ------------------------------------------------------------ providers
    def register(self, provider):
        api = getattr(provider, "api", None)
        if api != API:
            raise ExtensionError(f"The Pro pack speaks interface {api}, this application needs {API}.")
        self._providers.append(provider)
        return provider

    def load_installed(self, module_name=PACK_MODULE):
        """Use the installed Pro pack, if there is one. Never raises: a missing pack is the normal case, a broken one is reported in `problems`."""
        try:
            mod = importlib.import_module(module_name)
        except ModuleNotFoundError as e:
            if e.name != module_name:
                self.problems.append(f"The Pro pack could not be loaded: {e}")
            return None
        except Exception as e:      # noqa: BLE001 - a pack that fails to import must not stop the application
            self.problems.append(f"The Pro pack could not be loaded: {e}")
            return None
        try:
            if self._trust is not None and not self._trust(mod):
                self.problems.append("The Pro pack was refused (its signature is not valid).")
                return None
            return self.register(mod.provider())
        except ExtensionError as e:
            self.problems.append(str(e))
        except Exception as e:      # noqa: BLE001
            self.problems.append(f"The Pro pack could not be started: {e}")
        return None

    # ------------------------------------------------------------ what is available
    @property
    def present(self):
        return bool(self._providers)

    def _offer(self, feature):
        for p in self._providers:
            o = (p.offers() or {}).get(feature)
            if o is not None:
                return p, o
        return None, None

    def decide(self, feature):
        p, o = self._offer(feature)
        return Decision(feature, False) if p is None else Decision(feature, True, o.get("limit"))

    def ui_strings(self, code):
        """The interface texts (in the language `code`) of the messages that the Pro pack itself writes: {English template: translation}. The pack ships them, not Core."""
        out = {}
        for p in self._providers:
            fn = getattr(p, "ui_strings", None)
            if fn is not None:
                out.update(fn(code) or {})
        return out

    def versions(self):
        return [getattr(p, "version", "") for p in self._providers]

    # ------------------------------------------------------------ calling
    def call(self, feature, method, payload=None):
        p, _o = self._offer(feature)
        if p is None:
            raise ExtensionError(f"{feature} is not available.")
        out = p.call(feature, method, _json_roundtrip(payload or {}, "The request"))
        return _json_roundtrip(out, "The answer")


def default(trust=None):
    """The host of a normal start: loads the installed Pro pack (`lexo_pro`) when there is one."""
    host = ExtensionHost(trust)
    host.load_installed()
    return host
