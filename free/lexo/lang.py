"""Language packs: the rules of the checks that depend on the language of the translation.

Core checks (tags, variables, brackets, length, dialogue box) are language-neutral and live in `validator`.
What depends on the target language is collected in a `Pack`:

  repeat_ok          words that are normally doubled in speech, so "the same word twice" must not be reported
  check_repeats      the repeated-word check is only reliable when `repeat_ok` is known; off for the neutral pack
  number_words_ok    a digit written out in words ("14" -> "čtrnáct") is normal, so a missing number is only an info
  stem               reduces a word to a stem so that glossary terms are found in any inflected form

The pack of the open project is chosen by the `language` key of project.json (see layout.py and docs/PROJECT-FILE.md);
without it the neutral pack `und` is used. Adding a language = adding a `Pack` to PACKS.
Messages shown to the user are still Czech sentences; they move to message keys with the UI translation.
"""
from dataclasses import dataclass, field
from typing import Callable, FrozenSet

from .util import fold


def _stem_none(word):
    return fold(word)


def _stem_cut_two(word):
    """Crude stem for languages with rich endings: drop the last two letters of a longer word."""
    folded = fold(word)
    return folded[:max(3, len(folded) - 2)]


@dataclass(frozen=True)
class Pack:
    code: str
    name: str
    repeat_ok: FrozenSet[str] = field(default_factory=frozenset)
    check_repeats: bool = False
    number_words_ok: bool = False
    stem: Callable[[str], str] = _stem_none


CS = Pack(
    code="cs", name="Čeština",
    repeat_ok=frozenset({"to", "tak", "ne", "no", "ať", "jo", "hej", "ha", "ah", "se", "si", "že", "jak", "co", "ani", "už",
                         "ale", "čuk", "haha", "hihi", "ho", "bla", "ještě", "moc", "hodně", "hrozně", "pořád", "dobře",
                         "fajn", "ok", "hmm", "ach"}),
    check_repeats=True, number_words_ok=True, stem=_stem_cut_two,
)
NEUTRAL = Pack(code="und", name="Neutral")

PACKS = {p.code: p for p in (CS, NEUTRAL)}


def get(code):
    """The pack for a language code, or None when there is none."""
    return PACKS.get((code or "").lower())


_current = NEUTRAL


def use(pack):
    global _current
    _current = pack or NEUTRAL


def current():
    return _current
