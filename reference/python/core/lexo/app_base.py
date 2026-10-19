"""Exceptions and constants shared by the parts of the application (app.py)."""
import re

from . import features
from .messages import msg

# The only addresses the app will open in the browser (the UI sends just the id). Empty until the product has a website to point to;
# nothing here may belong to one person or one game.
LINKS = {}
# define de = Character("Derek", color="#0066ff")      /      define mc = DynamicCharacter("name", ...)
CHARACTER_RE = re.compile(r"""^\s*define\s+(?:\w+\.)?(\w+)\s*=\s*(Dynamic)?Character\s*\(\s*(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)')""")
CHARACTERS_MAX_BYTES = 16 * 1024 * 1024        # a game script may hold the definitions too, so this is not a "small list" limit any more
# First words of Ren'Py screen statements that appear in translated UI blocks; they look like speakers but are not.
SCREEN_KEYWORDS = {"text", "textbutton", "label", "tooltip", "hover", "idle", "insensitive", "timer", "key", "add",
                   "bar", "vbar", "image", "imagebutton", "use", "frame", "window", "vbox", "hbox", "fixed", "grid",
                   "mousearea", "input", "button", "action", "style"}
PAGE_LIMIT = 500
UI_PREF_MAX = 20000


class AppError(Exception):
    """An operation failed for a reason the user should read."""


class ProjectBusy(AppError):
    """The project is open in another running copy of the app (see lock.py)."""

    def __init__(self, message, path):
        super().__init__(message)
        self.path = str(path)


class FeatureLocked(AppError):
    """A feature of the Pro version was used without it (see features.py). `feature` is its catalog id."""

    def __init__(self, feature):
        self.feature = feature
        super().__init__(msg("“{title}” is a Pro feature.", title=msg(features.CATALOG[feature].title)))
