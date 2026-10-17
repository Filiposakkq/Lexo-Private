"""Publishing progress.json to the website repository — preview first, publish only on confirmation.

Generating the data (export.py) and publishing it are separate steps:

  plan(...)     read-only. Describes exactly what would happen: repository, branch, target file,
                what changes, the git commands, warnings (e.g. "push to main deploys the site").
  publish(...)  runs only with the token returned by plan() *and* the confirmed branch name.
                If the data or the repository changed after the preview, it refuses.

Safety properties:
  * nothing is pushed without publish(); there is no "push on exit" any more;
  * only the one data file is added and committed (other changes in the repo are untouched);
  * `git pull --ff-only` (no rebase / autostash) so the repo's working tree is never rewritten;
  * the file is written atomically and re-read to verify it;
  * when the push fails, the local commit and the file are rolled back, so a failed publish
    leaves the repository as it was.
"""
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path, PurePosixPath

from .util import WriteFailed, atomic_write_bytes
from .messages import msg

NOWIN = {"creationflags": 0x08000000} if sys.platform.startswith("win") else {}
COMMIT_MESSAGE = "chore(translation): update progress data"
PROTECTED_BRANCHES = ("main", "master")


class PublishError(Exception):
    pass


def _git(repo, *args, timeout=120):
    env = dict(os.environ, GIT_TERMINAL_PROMPT="0")
    cmd = ["git", *(["-C", str(repo)] if repo else []), *args]
    try:
        return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, env=env, **NOWIN)
    except FileNotFoundError:
        raise PublishError(msg("Git is not installed or not in PATH (https://git-scm.com).")) from None
    except subprocess.TimeoutExpired:
        raise PublishError(msg("Git did not answer in time (sign-in? network?).")) from None


def _tail(r):
    return (r.stderr or r.stdout or "").strip()[-300:]


def default_clone_dir(spec):
    name = spec.rstrip("/").split("/")[-1].removesuffix(".git") or "repo"
    return Path.home() / ".progress-web" / name


def repo_url(spec):
    spec = spec.strip()
    if re.fullmatch(r"[\w.-]+/[\w.-]+", spec) and not re.match(r"^[A-Za-z]:", spec):
        return f"https://github.com/{spec}.git"
    if re.match(r"^(https?://|git@|file:)", spec):
        return spec
    return None


def locate_repo(spec):
    """(path, url): a local folder if there is one, or the default clone folder and the URL to clone."""
    p = Path(spec).expanduser()
    if p.is_dir():
        return p, None
    url = repo_url(spec)
    if url is None:
        return None, None
    return default_clone_dir(url), url


def clean_target(rel):
    """Validate the target path inside the repository (relative, no '..', a .json file)."""
    rel = str(rel).replace("\\", "/").strip()
    pp = PurePosixPath(rel)
    if not rel or pp.is_absolute() or ".." in pp.parts or re.match(r"^[A-Za-z]:", rel) or pp.suffix.lower() != ".json":
        raise PublishError(msg("Invalid path of the file in the repository: {rel} (it must be a relative path to a .json file).", rel=rel))
    return str(pp)


def canonical(data):
    """Content identity for the confirmation token: the data without its timestamp."""
    d = dict(data)
    d.pop("updated", None)
    return json.dumps(d, ensure_ascii=False, sort_keys=True)


def _summ(d):
    try:
        return {"translation": d["translation"]["percent"], "proofreading": d["proofreading"]["percent"],
                "updated": d.get("updated")}
    except (KeyError, TypeError):
        return None


def plan(web, data, text):
    """Read-only description of a publish. `web` is the config's web section, `text` the JSON text."""
    if not web.get("repo"):
        raise PublishError(msg("The website is not set up – fill in the repository in Settings."))
    target = clean_target(web.get("path", ""))
    path, url = locate_repo(web["repo"])
    if path is None:
        raise PublishError(msg("The repository “{repo}” is neither a folder nor a GitHub address.", repo=web['repo']))
    info = {"repo": web["repo"], "repo_path": str(path), "url": url, "target": target,
            "will_clone": url is not None and not (path / ".git").is_dir(),
            "branch": "", "remote": "", "upstream": "", "dirty": [], "current": None,
            "changed": True, "blockers": [], "warnings": [], "commands": [],
            "new": _summ(data), "live": False}
    info["token"] = hashlib.sha256((canonical(data) + "|" + str(path) + "|" + target).encode()).hexdigest()[:16]
    if info["will_clone"]:
        info["warnings"].append(msg("The repository is not downloaded yet. When publishing, it is cloned from {url} and the data is sent to its default branch (usually main = the live website).", url=url))
        info["live"] = True            # the default branch of a new clone is usually the live website
        info["commands"] = [f"git clone {url} \"{path}\"", f"write {target}", f"git add -- {target}",
                            f"git commit -m \"{COMMIT_MESSAGE}\" -- {target}", "git push"]
        return info
    if not (path / ".git").exists():
        raise PublishError(msg("The folder is not a git repository: {path}", path=path))
    branch = _git(path, "rev-parse", "--abbrev-ref", "HEAD")
    if branch.returncode != 0:
        raise PublishError(msg("The git repository cannot be read: {output}", output=_tail(branch)))
    info["branch"] = branch.stdout.strip()
    if info["branch"] == "HEAD":
        info["blockers"].append(msg("The repository is in a detached HEAD – switch to a branch."))
    up = _git(path, "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}")
    if up.returncode == 0:
        info["upstream"] = up.stdout.strip()
    else:
        info["blockers"].append(msg("The branch {branch} has no remote branch (upstream) set – a push would go nowhere.", branch=info['branch']))
    rem = _git(path, "remote", "get-url", "origin")
    info["remote"] = rem.stdout.strip() if rem.returncode == 0 else ""
    st = _git(path, "status", "--porcelain")
    if st.returncode == 0:
        info["dirty"] = [l[3:] for l in st.stdout.splitlines() if l[3:].replace("\\", "/") != target][:20]
    f = path / target
    if f.is_file():
        try:
            cur = json.loads(f.read_text(encoding="utf-8-sig"))
            info["current"] = _summ(cur)
            info["changed"] = canonical(cur) != canonical(data)
        except (OSError, ValueError):
            info["warnings"].append(msg("The existing file in the repository cannot be read; it will be overwritten."))
    else:
        info["warnings"].append(msg("The file in the repository does not exist yet, it will be created."))
    if info["branch"] in PROTECTED_BRANCHES:
        info["warnings"].append(msg("A push to the branch {branch} deploys the website live (Cloudflare Workers).", branch=info['branch']))
        info["live"] = True
    if info["dirty"]:
        info["warnings"].append(msg("There are other uncommitted changes in the repository ({n}); publishing does not touch them.", n=len(info['dirty'])))
    if not info["changed"]:
        info["warnings"].append(msg("The published data does not differ from the current data (except the time) – nothing to publish."))
    info["commands"] = ["git pull --ff-only", f"write {target}", f"git add -- {target}",
                        f"git commit -m \"{COMMIT_MESSAGE}\" -- {target}", "git push"]
    return info


def publish(web, data, text, token, confirmed_branch):
    """Run the publish. Raises PublishError (nothing or only a reverted state left behind)."""
    info = plan(web, data, text)
    if token != info["token"]:
        raise PublishError(msg("The data or the repository changed since the preview – open the preview again."))
    if info["blockers"]:
        raise PublishError(" ".join(info["blockers"]))
    path, target = Path(info["repo_path"]), info["target"]
    log = []
    if info["will_clone"]:
        if confirmed_branch != "*clone*":
            raise PublishError(msg("Cloning and publishing was not confirmed."))
        path.parent.mkdir(parents=True, exist_ok=True)
        r = _git(None, "clone", info["url"], str(path), timeout=600)
        if r.returncode != 0:
            raise PublishError("git clone selhal: " + _tail(r))
        log.append(msg("Repository cloned."))
        info = plan(web, data, text)
        if info["blockers"]:
            raise PublishError(" ".join(info["blockers"]))
    elif confirmed_branch != info["branch"]:
        raise PublishError(msg("The confirmed branch “{a}” does not match the current one “{b}”.", a=confirmed_branch, b=info['branch']))
    if not info["changed"] and not info["will_clone"]:
        return {"ok": True, "message": msg("No change – the website is already up to date."), "log": log}

    r = _git(path, "pull", "--ff-only", timeout=300)
    if r.returncode != 0:
        raise PublishError(msg("git pull --ff-only failed (nothing changed): {output}", output=_tail(r)))
    dest = path / target
    before = dest.read_bytes() if dest.is_file() else None
    payload = text.encode("utf-8")
    try:
        json.loads(payload.decode("utf-8"))
        atomic_write_bytes(dest, payload)
        if json.loads(dest.read_text(encoding="utf-8")).get("schema") != data.get("schema"):
            raise PublishError(msg("The written file did not pass the check."))
    except (WriteFailed, ValueError) as e:
        _restore(dest, before)
        raise PublishError(msg("The file in the repository cannot be written: {reason}", reason=e)) from e
    except PublishError:
        _restore(dest, before)
        raise

    def rollback(sha=None):
        if sha:
            head = _git(path, "rev-parse", "HEAD").stdout.strip()
            if head == sha:
                _git(path, "reset", "--soft", "HEAD~1")
        _git(path, "reset", "-q", "--", target)
        _restore(dest, before)

    r = _git(path, "add", "--", target)
    if r.returncode != 0:
        rollback()
        raise PublishError("git add selhal: " + _tail(r))
    if _git(path, "diff", "--cached", "--quiet", "--", target).returncode == 0:
        rollback()
        return {"ok": True, "message": msg("No change – the website is already up to date."), "log": log}
    r = _git(path, "commit", "-m", COMMIT_MESSAGE, "--", target)
    if r.returncode != 0:
        rollback()
        raise PublishError(msg("git commit failed (the file was restored to its previous state): {output}", output=_tail(r)))
    sha = _git(path, "rev-parse", "HEAD").stdout.strip()
    r = _git(path, "push", timeout=300)
    if r.returncode != 0:
        rollback(sha)
        raise PublishError(msg("git push failed. The local commit and the file were restored to their previous state: {output}", output=_tail(r)))
    return {"ok": True, "message": msg("Published to the branch {branch} ({sha}).", branch=info['branch'] or msg("default"), sha=sha[:7]),
            "log": log, "commit": sha}


def _restore(dest, before):
    try:
        if before is None:
            if dest.exists():
                dest.unlink()
        else:
            atomic_write_bytes(dest, before)
    except OSError:
        pass
