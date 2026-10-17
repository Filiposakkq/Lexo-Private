"""Tools that prepare a game for translating: unpack .rpa archives, decompile .rpyc scripts, generate the translation files.

  extract_rpa            unpack an archive natively (the format is simple; this stays in the app, also after the Rust port)
  copy_scripts           copy the compiled scripts (.rpyc / .rpymc) of a game into a workspace folder
  decompile              run unrpyc (an EXTERNAL helper, never re-implemented) on a workspace folder
  generate_translation   let the Ren'Py SDK create game/tl/<language> (`renpy <game> translate <language>`)

Rules that hold for all of them:
  - The game folder is only READ. Anything written goes to a workspace folder the caller chooses. The one exception is
    `generate_translation`, whose product (`game/tl/<language>`) lives inside the project it is run for; it refuses to run
    unless the caller says so explicitly (`write_into_project=True`) and never overwrites an existing language folder.
  - Archive entries cannot write outside the target folder (no `..`, no absolute paths, no drive letters).
  - External programs are started with an argument list (no shell) and a time limit; their output is kept and shown.
  - The pieces that start programs take a `runner(argv, cwd) -> (exit code, text)` so tests do not need Ren'Py.

unrpyc comes in two lines: v2 needs Python 3.9+ and is for Ren'Py 8 games, v1 (legacy branch) runs on the Python 2.7 of a Ren'Py 7
SDK. The caller picks the script and the Python; `lint.find_python(sdk)` gives the one that ships with an SDK.
"""
import os
import re
import shutil
import subprocess
import time
from pathlib import Path

from . import lint, rpa
from .messages import msg

NOWIN = lint.NOWIN
TIMEOUT = 1800
MAX_OUTPUT = 200_000
CHUNK = 1024 * 1024
DEFAULT_MAX_FILE = 2 * 1024 ** 3            # one file inside an archive
DEFAULT_MAX_TOTAL = 16 * 1024 ** 3          # everything one extraction writes
LANGUAGE_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")        # what Ren'Py accepts as a language name
SCRIPT_SUFFIXES = (".rpyc", ".rpymc")


class ToolError(Exception):
    pass


# ---------------------------------------------------------------- archives
def find_archives(game_dir):
    """The .rpa archives directly in or below `game_dir`, sorted."""
    return sorted(p for p in Path(game_dir).rglob("*.rpa") if p.is_file())


def safe_target(dest, name):
    """`dest / name` when `name` stays inside `dest`, otherwise None. Handles `..`, absolute paths, drive letters and NUL."""
    if not name or "\0" in name:
        return None
    norm = name.replace("\\", "/")
    if norm.startswith("/") or re.match(r"^[A-Za-z]:", norm):
        return None
    parts = [p for p in norm.split("/") if p not in ("", ".")]
    if not parts or any(p == ".." for p in parts):
        return None
    dest = Path(dest).resolve()
    target = (dest.joinpath(*parts)).resolve()
    return target if dest == target.parent or dest in target.parents else None


def list_archive(archive):
    """[(name, size)] of an archive, sorted by name. Reads only the index."""
    _ver, index = rpa.read_index(archive)
    return sorted((n, ln) for n, (_off, ln, _pfx) in index.items())


def extract_rpa(archive, dest, only=None, overwrite=False, max_file=DEFAULT_MAX_FILE, max_total=DEFAULT_MAX_TOTAL):
    """Unpack `archive` into `dest`. `only` = optional predicate on the entry name. Existing files are kept unless `overwrite`.
    Returns {written: [names], skipped: [(name, reason)], bytes}. Raises rpa.RpaError for an unusable archive and ToolError
    when `dest` is a file."""
    dest = Path(dest)
    if dest.exists() and not dest.is_dir():
        raise ToolError(msg("The target is not a folder: {dest}", dest=dest))
    _ver, index = rpa.read_index(archive)
    dest.mkdir(parents=True, exist_ok=True)
    written, skipped, total = [], [], 0
    with open(archive, "rb") as src:
        for name in sorted(index):
            offset, length, prefix = index[name]
            if only is not None and not only(name):
                continue
            target = safe_target(dest, name)
            if target is None:
                skipped.append((name, msg("the path would lead outside the target folder")))
                continue
            if length > max_file:
                skipped.append((name, msg("the file is too large")))
                continue
            if total + length > max_total:
                skipped.append((name, msg("the total limit of the extraction was exceeded")))
                continue
            if target.exists() and not overwrite:
                skipped.append((name, msg("the file already exists")))
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            tmp = target.with_name(target.name + ".part")
            try:
                with open(tmp, "wb") as out:
                    out.write(prefix)
                    src.seek(offset)
                    left = max(0, length - len(prefix))
                    while left:
                        chunk = src.read(min(CHUNK, left))
                        if not chunk:
                            raise OSError(msg("the archive ends earlier than the index promises"))
                        out.write(chunk)
                        left -= len(chunk)
                tmp.replace(target)
            except OSError as e:
                tmp.unlink(missing_ok=True)
                skipped.append((name, msg("cannot be written: {reason}", reason=e.strerror or e)))
                continue
            written.append(name)
            total += length
    return {"written": written, "skipped": skipped, "bytes": total}


# ---------------------------------------------------------------- workspace for a built game
LINK_SKIP_ROOT = {"_progress_data", ".lexo"}        # data of translation tools lying next to the game: never part of a workspace
LINK_SKIP_DIRS = {"cache", "saves", "tl"}            # Ren'Py rewrites these; they are created fresh in the workspace
COPY_SUFFIXES = {".rpy", ".rpyc", ".rpymc", ".rpyb", ".pyc", ".pyo"}   # files the engine may rewrite in place: copied, never linked


def make_workspace(game_root, workspace):
    """Build a working copy of a built game that costs (almost) no space: every file is a hard link to the original, except the
    folders Ren'Py rewrites (cache, saves, tl: not copied) and the script/bytecode files (copied, so a rewrite cannot reach the
    original). Falls back to a plain copy of a file when a link is impossible (another drive), which can be large.
    The original is only read. Returns {linked, copied}."""
    game_root, workspace = Path(game_root).resolve(), Path(workspace).resolve()
    if workspace == game_root or game_root in workspace.parents or workspace in game_root.parents:
        raise ToolError(msg("The workspace must not be inside the game folder."))
    if not (game_root / "game").is_dir():
        raise ToolError(msg("There is no game subfolder in the folder: {base}", base=game_root))
    linked = copied = 0
    for src in sorted(game_root.rglob("*")):
        rel = src.relative_to(game_root)
        if rel.parts[0] in LINK_SKIP_ROOT or (len(rel.parts) > 1 and rel.parts[0] == "game" and rel.parts[1] in LINK_SKIP_DIRS):
            continue
        if src.is_dir():
            (workspace / rel).mkdir(parents=True, exist_ok=True)
            continue
        if not src.is_file():
            continue
        dst = workspace / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        if dst.exists():
            continue
        if src.suffix.lower() not in COPY_SUFFIXES:
            try:
                os.link(src, dst)
                linked += 1
                continue
            except OSError:
                pass
        shutil.copy2(src, dst)
        copied += 1
    return {"linked": linked, "copied": copied}


# ---------------------------------------------------------------- decompiling
def copy_scripts(game_dir, workspace):
    """Copy every .rpyc / .rpymc below `game_dir` into `workspace`, keeping the relative paths. The game is only read.
    Returns the number of copied files."""
    game_dir, workspace = Path(game_dir).resolve(), Path(workspace).resolve()
    if workspace == game_dir or game_dir in workspace.parents:
        raise ToolError(msg("The workspace must not be inside the game folder."))
    n = 0
    for f in sorted(game_dir.rglob("*")):
        if f.is_file() and f.suffix.lower() in SCRIPT_SUFFIXES:
            target = workspace / f.relative_to(game_dir)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(f, target)
            n += 1
    return n


def _run_process(argv, cwd, timeout=TIMEOUT):
    try:
        p = subprocess.run(argv, cwd=str(cwd), capture_output=True, timeout=timeout, **NOWIN)
    except subprocess.TimeoutExpired as e:
        raise ToolError(msg("The command took longer than {timeout} s and was stopped.", timeout=timeout)) from e
    except OSError as e:
        raise ToolError(msg("The command could not be started: {reason}", reason=e.strerror or e)) from e
    return p.returncode, (p.stdout or b"").decode("utf-8", errors="replace") + (p.stderr or b"").decode("utf-8", errors="replace")


def build_unrpyc_command(python, unrpyc_script, target, clobber=False, try_harder=False, no_init_offset=False):
    python, script, target = Path(python), Path(unrpyc_script), Path(target)
    if not python.is_file():
        raise ToolError(msg("The Python for unrpyc does not exist: {python}", python=python))
    if not script.is_file():
        raise ToolError(msg("The script unrpyc.py does not exist: {script}", script=script))
    if not target.is_dir():
        raise ToolError(msg("The workspace folder does not exist: {target}", target=target))
    argv = [str(python), str(script)]
    if clobber:
        argv.append("-c")
    if try_harder:
        argv.append("--try-harder")
    if no_init_offset:
        argv.append("--no-init-offset")
    argv.append(str(target))
    return argv, script.parent


def decompile(python, unrpyc_script, workspace, clobber=False, try_harder=False, no_init_offset=False, runner=None):
    """Run unrpyc on `workspace` (a folder with copied .rpyc files; see copy_scripts). Returns
    {ok, exit_code, raw, written: [relative .rpy paths that did not exist before], seconds}."""
    workspace = Path(workspace)
    argv, cwd = build_unrpyc_command(python, unrpyc_script, workspace, clobber, try_harder, no_init_offset)
    before = {p for p in workspace.rglob("*.rpy")}
    t0 = time.time()
    code, text = (runner or _run_process)(argv, cwd)
    after = {p for p in workspace.rglob("*.rpy")}
    written = sorted(str(p.relative_to(workspace)).replace("\\", "/") for p in after - before)
    return {"ok": code == 0, "exit_code": code, "raw": text[:MAX_OUTPUT], "written": written, "seconds": round(time.time() - t0, 1)}


# ---------------------------------------------------------------- the whole way from a built game to translation files
SCRIPT_SOURCE_SUFFIXES = (".rpy", ".rpyc", ".rpym", ".rpymc")


def extract_scripts(workspace):
    """Unpack the scripts that sit inside the .rpa archives of the workspace's game folder into that folder (next to the archives).
    Returns {archives: n, written: n, skipped: n}. Existing files are kept."""
    game = Path(workspace) / "game"
    done = {"archives": 0, "written": 0, "skipped": 0}
    for arc in sorted(game.glob("*.rpa")):
        try:
            names = [n for n, _ in list_archive(arc) if n.lower().endswith(SCRIPT_SOURCE_SUFFIXES)]
        except rpa.RpaError:
            continue
        if not names:
            continue
        wanted = set(names)
        res = extract_rpa(arc, game, only=wanted.__contains__)
        done["archives"] += 1
        done["written"] += len(res["written"])
        done["skipped"] += len(res["skipped"])
    return done


def repair_continuations(text):
    """unrpyc loses the indentation of the lines that follow a line ending in a backslash inside a python block, which makes Ren'Py
    refuse the file. Indent such lines to the block's level. Returns (new text, number of lines changed); other text is left alone."""
    out, changed, in_block, cont = [], 0, False, False
    for line in text.split("\n"):
        top = bool(line.strip()) and not line.startswith((" ", "\t"))
        if cont and top:
            line, changed = "    " + line, changed + 1
        elif top:
            in_block = line.rstrip().endswith(":") and "python" in line
        cont = in_block and line.rstrip().endswith("\\")
        out.append(line)
    return "\n".join(out), changed


def repair_decompiled(game_dir, files):
    """Apply repair_continuations to the decompiled files (relative paths under game_dir). Returns {file: lines changed} for the ones that changed."""
    done = {}
    for rel in files:
        path = Path(game_dir) / rel
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        new, n = repair_continuations(text)
        if n:
            path.write_text(new, encoding="utf-8", newline="\n")
            done[rel] = n
    return done


PARSE_ERROR_RE = re.compile(r'File "game/(?P<file>[^"]+\.rpym?)", line \d+')


def quarantine_unparsable(game_dir, raw, protect=()):
    """Move aside (rename to `<name>.unparsable`) the .rpy files that Ren'Py's output names as having errors, so the rest can still be
    translated. This includes decompiled translations that a game ships inside its archives (game/tl/russian/...). `protect` = relative
    prefixes that are never touched (the folder being generated). Returns the list of moved relative paths."""
    moved = []
    for rel in sorted({m.group("file") for m in PARSE_ERROR_RE.finditer(raw or "")}):
        path = Path(game_dir) / rel
        if path.is_file() and not rel.startswith(tuple(protect)):
            path.replace(path.with_name(path.name + ".unparsable"))
            moved.append(rel)
    return moved


def prepare_translation(game_root, workspace, language, unrpyc_python, unrpyc_script, runner=None, progress=None):
    """Everything between "a built game" and "game/tl/<language> exists", in a workspace; the game itself is only read:
    1. make_workspace (links), 2. extract_scripts (from the archives), 3. decompile (unrpyc) so that every script has a .rpy,
    4. generate_translation with the game's own engine. `progress(step_name)` is called before each step. Returns a dict with the
    result of each step and `ok`. Stops at the first failing step."""
    note = progress or (lambda _step: None)
    out = {"ok": False, "workspace": str(workspace)}
    note("workspace")
    out["workspace_files"] = make_workspace(game_root, workspace)
    note("extract")
    out["extract"] = extract_scripts(workspace)
    note("decompile")
    game = Path(workspace) / "game"
    out["decompile"] = decompile(unrpyc_python, unrpyc_script, game, clobber=False, try_harder=True, runner=runner)
    if not out["decompile"]["ok"]:
        return out
    out["repaired"] = repair_decompiled(game, out["decompile"]["written"])
    note("generate")
    out["quarantined"] = []
    for _attempt in range(3):
        out["generate"] = generate_translation(workspace, workspace, language, write_into_project=True, runner=runner)
        if out["generate"]["ok"]:
            break
        moved = quarantine_unparsable(game, out["generate"]["raw"], ("tl/" + language + "/",))
        if not moved:
            break
        out["quarantined"] += moved
        tl = game / "tl" / language
        if tl.is_dir() and not any(tl.rglob("*.rpy")):                  # the failed run left an empty folder
            shutil.rmtree(tl, ignore_errors=True)
    out["ok"] = out["generate"]["ok"]
    return out


# ---------------------------------------------------------------- translation files
def plan_translation(base, language):
    """What generate_translation would do, without doing it: {folder, exists}."""
    if not LANGUAGE_RE.fullmatch(language or ""):
        raise ToolError(msg("The language name may contain only letters, digits and an underscore and must not start with a digit."))
    folder = Path(base) / "game" / "tl" / language
    return {"folder": str(folder), "exists": folder.exists()}


def generate_translation(sdk, base, language, write_into_project=False, runner=None):
    """Let the Ren'Py SDK generate game/tl/<language> for the project `base` (the folder that contains game/).
    It writes INTO the project, so the caller must pass write_into_project=True, and an existing language folder is never touched."""
    plan = plan_translation(base, language)
    if not (Path(base) / "game").is_dir():
        raise ToolError(msg("There is no game subfolder in the folder: {base}", base=base))
    if not write_into_project:
        raise ToolError(msg("Generating writes into the project (game/tl/{language}); it must be confirmed explicitly.", language=language))
    if plan["exists"]:
        raise ToolError(msg("The folder {folder} already exists and I will not overwrite it.", folder=plan['folder']))
    try:
        argv, cwd = lint.build_command(sdk, base, ("translate", language))
    except lint.LintError as e:
        raise ToolError(str(e)) from e
    t0 = time.time()
    code, text = (runner or _run_process)(argv, cwd)
    folder = Path(plan["folder"])
    files = sorted(str(p.relative_to(folder)).replace("\\", "/") for p in folder.rglob("*.rpy")) if folder.is_dir() else []
    return {"ok": code == 0 and bool(files), "exit_code": code, "raw": text[:MAX_OUTPUT], "folder": plan["folder"], "files": files,
            "seconds": round(time.time() - t0, 1)}
