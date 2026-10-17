"""Application state and operations (no HTTP, no UI) — everything the server exposes lives here.

`App` is assembled from parts by area (app_*.py); each part is a mixin with the methods of one area. Exceptions and shared constants are in app_base.py.
"""
from .app_core import CorePart
from .app_validation import ValidationPart
from .app_entries import EntriesPart
from .app_glossary import GlossaryPart
from .app_proof import ProofPart
from .app_assign import AssignPart
from .app_memory import MemoryPart
from .app_game import GamePart
from .app_snapshots import SnapshotsPart
from .app_lint import LintPart
from .app_fixes import FixesPart
from .app_mt import MtPart
from .app_publish import PublishPart
from .app_missed import MissedPart
from .app_menu import MenuPart
from .app_base import AppError, CHARACTERS_MAX_BYTES, CHARACTER_RE, FeatureLocked, LINKS, PAGE_LIMIT, ProjectBusy, SCREEN_KEYWORDS, UI_PREF_MAX  # noqa: F401  (re-exported: the server, the CLI and the tests import them from here)


class App(CorePart, ValidationPart, EntriesPart, GlossaryPart, ProofPart, AssignPart, MemoryPart, GamePart, SnapshotsPart, LintPart, FixesPart, MtPart, PublishPart, MissedPart, MenuPart):
    """The application: all parts together; state is created in CorePart.__init__."""
