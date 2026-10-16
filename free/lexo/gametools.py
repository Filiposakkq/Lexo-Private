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
import re
import shutil
import subprocess
import sys
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
                skipped.append((name, f"nejde zapsat: {e.strerror or e}"))
                continue
            written.append(name)
            total += length
    return {"written": written, "skipped": skipped, "bytes": total}


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
