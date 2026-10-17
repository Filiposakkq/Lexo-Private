"""Menus of a Ren'Py script: which lines are choices, what question they answer and what follows each of them.

A menu in the game script looks like

    menu:
        "What do you say?"           <- optional question (a line of dialogue)
        "Invite her" if met_her:     <- a choice, maybe with a condition
            mc "Coffee?"             <- its body: the first lines of dialogue show what the choice means
            jump cafe
        "Leave":
            ...

The translation of a choice sits in `translate <lang> strings:` (far from the dialogue), so the translator needs the menu around it.
This module only reads the script text; it finds nothing it cannot see (no jumps, no `call screen`, no menus built in Python).
"""
import re

OPTION_RE = re.compile(r"""^\s*(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)')\s*(?:\bif\b\s+(.+?))?\s*:\s*(?:#.*)?$""")
MENU_RE = re.compile(r"^\s*menu(?:\s+\w+)?\s*(?:\(.*\))?\s*:\s*(?:#.*)?$")
# a line of dialogue: `"text"`, or `who "text"`, or `who attr attr "text" with x`; not a statement that merely contains a string
SAY_RE = re.compile(r"""^\s*(?:[A-Za-z_][\w.]*(?:\s+[\w@.]+)*\s+)?(?:"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')\s*(?:with\s+.+)?$""")
NOT_SAY = {"if", "elif", "else", "while", "for", "show", "scene", "hide", "play", "stop", "queue", "with", "image", "define", "default", "label", "jump", "call",
           "return", "pass", "python", "init", "screen", "transform", "style", "translate", "window", "pause", "voice", "renpy", "camera", "text", "menu", "$"}
AFTER_LINES = 2           # lines of dialogue shown after each choice
BEFORE_LINES = 2          # lines of dialogue shown before a menu that has no question of its own
LOOK_BACK = 60


def indent_of(text):
    return len(text) - len(text.lstrip(" \t"))


def blank(text):
    s = text.strip()
    return not s or s.startswith("#")


def is_say(text):
    if blank(text) or not SAY_RE.match(text):
        return False
    first = text.strip().split(None, 1)[0].rstrip(":").lower()
    return first not in NOT_SAY


def literal_of(text):
    """The text between the quotes of the first string literal of a line."""
    m = re.search(r"""(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)')""", text)
    return (m.group(1) if m.group(1) is not None else m.group(2)) if m else ""


def find_menu(lines, line):
    """The menu whose choice stands on the 1-based `line` of the script, or None.
    {menu_line, question:[line numbers], before:[line numbers], options:[{n, text, cond, after:[line numbers]}], current: index of the choice}."""
    if not 1 <= line <= len(lines) or not OPTION_RE.match(lines[line - 1]):
        return None
    level = indent_of(lines[line - 1])
    head = None
    for i in range(line - 2, max(-1, line - 2 - 400), -1):          # up to the `menu:` line: the first line with less indent
        if blank(lines[i]):
            continue
        if indent_of(lines[i]) < level:
            head = i if MENU_RE.match(lines[i]) else None
            break
    if head is None:
        return None
    question, options = [], []
    for i in range(head + 1, len(lines)):
        text = lines[i]
        if blank(text):
            continue
        ind = indent_of(text)
        if ind < level:
            break
        if ind == level:
            m = OPTION_RE.match(text)
            if m:
                options.append({"n": i + 1, "text": m.group(1) if m.group(1) is not None else m.group(2), "cond": m.group(3) or "", "after": []})
            elif not options and is_say(text):
                question.append(i + 1)
        elif options and len(options[-1]["after"]) < AFTER_LINES:
            if MENU_RE.match(text):                                   # a menu inside a choice: its lines are not this choice's
                options[-1]["after"].append(0)
            elif is_say(text) and 0 not in options[-1]["after"]:
                options[-1]["after"].append(i + 1)
    for o in options:
        o["after"] = [n for n in o["after"] if n]
    before = []
    if not question:                                                    # a menu without a question: the lines said just before it give the context
        for i in range(head - 1, max(-1, head - 1 - LOOK_BACK), -1):
            text = lines[i]
            if blank(text):
                continue
            if is_say(text):
                before.insert(0, i + 1)
                if len(before) >= BEFORE_LINES:
                    break
            elif indent_of(text) < indent_of(lines[head]) or MENU_RE.match(text) or re.match(r"\s*label\b", text):
                break
    cur = next((k for k, o in enumerate(options) if o["n"] == line), -1)
    return {"menu_line": head + 1, "question": question, "before": before, "options": options, "current": cur}
