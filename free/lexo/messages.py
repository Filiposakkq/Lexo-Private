"""User-facing messages of the Python side.

A message is written once, in English, with {placeholders}:  msg("Cannot save {name}: {reason}", name=n, reason=r).
`str(message)` is the formatted English text (logs, the command line, tests). On the way to the browser the server sends the template and the
values (see `wire`), and the browser translates the template with the interface language pack (ui.json) and fills the values, exactly like
the texts of the UI itself (docs/UI-TEXTS.md).

Rules: the template is a plain string literal (never built by concatenation) so that it can be found and translated; values go into
placeholders, also tags such as "{/b}" (pass them as values: tag="{/b}"), because a literal brace would look like a placeholder.
A message that is shown to the user must be a `Message`; a plain `str` is shown as it is, untranslated.
"""
import re

PLACEHOLDER = re.compile(r"\{(\w+)\}")


class Message(str):
    """The formatted English text that also remembers its template and its values."""

    def __new__(cls, template, plural=None, **params):
        one = plural is None or params.get("n") == 1
        text = PLACEHOLDER.sub(lambda m: str(params[m.group(1)]) if m.group(1) in params else m.group(0), template if one else plural)
        obj = super().__new__(cls, text)
        obj.template = template
        obj.plural = plural                      # the template for other numbers than 1 (counted messages), or None
        obj.params = params
        return obj

    def __reduce__(self):                       # copy / pickle keep the template
        return (_rebuild, (self.template, self.plural, self.params))


def _rebuild(template, plural, params):
    return Message(template, plural, **params)


def msg(template, **params):
    return Message(template, None, **params)


def msgn(n, one, many, **params):
    """A counted message: `one` is used for n == 1, `many` otherwise; both contain {n}. The language pack gives one text per plural form."""
    return Message(one, many, n=n, **params)


def text_of(exc):
    """The message of an exception: its Message when it has one (so the template survives), otherwise str(exc)."""
    first = exc.args[0] if getattr(exc, "args", None) else None
    return first if isinstance(first, Message) else str(exc)


def wire(value):
    """JSON-ready copy of `value`: every Message becomes {"$m": template, "p": values} (and "$n": the plural template for counted
    messages); containers are walked, other values stay."""
    if isinstance(value, Message):
        out = {"$m": value.template, "p": {k: wire(v) for k, v in value.params.items()}}
        if value.plural is not None:
            out["$n"] = value.plural
        return out
    if isinstance(value, dict):
        return {k: wire(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [wire(v) for v in value]
    return value


def render(value):
    """The English text of what `wire` produced: every {"$m": ..., "p": ...} becomes its formatted English text. For tests and the command line."""
    if isinstance(value, dict):
        if "$m" in value:
            params = {k: render(v) for k, v in value.get("p", {}).items()}
            return str(Message(value["$m"], value.get("$n"), **params))
        return {k: render(v) for k, v in value.items()}
    if isinstance(value, list):
        return [render(v) for v in value]
    return value
