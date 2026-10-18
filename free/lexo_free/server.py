"""Local HTTP server: serves the UI (lexo/ui) and a small JSON API on 127.0.0.1.

Only the loopback interface is bound, the Host header is checked (DNS-rebinding protection)
and every POST needs the per-run token that the page receives from the server.
"""
import json
import re
import mimetypes
import secrets
import socket
import threading
import time
import traceback
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlencode, urlparse

from lexo import api
from lexo.messages import msg, text_of, wire
from lexo.app import AppError, ProjectBusy
from lexo.config import config_dir
from lexo.project import check_project_dir
from lexo.util import WriteFailed

UI_DIR = Path(__file__).resolve().parent / "ui"
MAX_BODY = 5 * 1024 * 1024


def log_exception():
    try:
        d = config_dir()
        d.mkdir(parents=True, exist_ok=True)
        with open(d / "error.log", "a", encoding="utf-8") as fh:
            fh.write(f"--- {time.strftime('%Y-%m-%d %H:%M:%S')}\n{traceback.format_exc()}\n")
    except OSError:
        pass


class Session:
    """Per-run server state."""

    def __init__(self, app, pick_folder=None):
        self.app = app
        self.token = secrets.token_hex(16)
        self.last_ping = time.time()
        self.bye_at = None
        self.pick_folder = pick_folder
        self.windows = {}              # window id -> time of its last ping (the app quits when the last one is gone)
        self.open_window = None        # set by cli: callable(url) that opens one more app window
        self.toggle_fullscreen = None  # set by cli: callable() -> bool, full screen of the window in front
        self.base_url = ""
        self.focus = {"seq": 0, "file": "", "line": 0, "text": None}   # the line open in the main window; companion windows follow it
        self.focus_cv = threading.Condition()
        self.companions = {}           # detail windows -> time of their last ping (the main window slims down while one is open)


def make_handler(sess):
    app = sess.app

    def q1(query, key, default=""):
        return (query.get(key) or [default])[0]

    class Handler(BaseHTTPRequestHandler):
        server_version = "Lexo"
        # Large single send() calls stalled intermittently on Windows loopback (the client got exactly
        # 65,280 bytes and then nothing), so bodies are written in small chunks.
        CHUNK = 16 * 1024

        def setup(self):
            super().setup()
            try:
                self.request.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            except OSError:
                pass

        def log_message(self, *args):
            pass

        # ---------------------------------------------------------- helpers
        def _host_ok(self):
            host = (self.headers.get("Host") or "").split(":")[0].lower()
            return host in ("127.0.0.1", "localhost", "[::1]")

        def _send(self, code, body, ctype, cache=False):
            data = body if isinstance(body, bytes) else body.encode("utf-8")
            self.send_response(code)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", "private, max-age=600" if cache else "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.send_header("Content-Security-Policy",
                             "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:")
            self.end_headers()
            for i in range(0, len(data), self.CHUNK):
                self.wfile.write(data[i:i + self.CHUNK])

        def _drain(self):
            """Read and discard the request body before an early reply. Closing a socket that still has unread
            data makes Windows reset the connection, so the client sometimes never sees the 403 / 413."""
            try:
                n = int(self.headers.get("Content-Length", 0))
                if 0 < n <= MAX_BODY:
                    self.rfile.read(n)
                elif n > MAX_BODY:
                    self.close_connection = True
            except (ValueError, OSError):
                self.close_connection = True

        def _json(self, payload, code=200):
            self._send(code, json.dumps(wire(payload), ensure_ascii=False), "application/json; charset=utf-8")

        def _static(self, name):
            path = (UI_DIR / name).resolve()
            if UI_DIR.resolve() not in path.parents or not path.is_file():
                return self._send(404, "not found", "text/plain")
            data = path.read_bytes()
            if name == "index.html":
                data = data.replace(b"__TOKEN__", sess.token.encode())
            ctype = mimetypes.guess_type(name)[0] or "application/octet-stream"
            if ctype.startswith("text/") or ctype in ("application/javascript", "application/json"):
                ctype += "; charset=utf-8"
            self._send(200, data, ctype)

        def _run(self, fn):
            try:
                with app.lock:
                    result = fn()
                self._json(result if isinstance(result, dict) else {"ok": True, "result": result})
            except ProjectBusy as e:
                self._json({"ok": False, "error": text_of(e), "locked": True, "path": e.path}, 409)
            except AppError as e:
                self._json({"ok": False, "error": text_of(e)}, 400)
            except KeyError as e:
                self._json({"ok": False, "error": msg("Missing value: {e}", e=e)}, 400)
            except WriteFailed as e:
                self._json({"ok": False, "error": msg("Cannot save: {reason} ({name})", reason=e.reason, name=e.path.name)}, 409)
            except Exception as e:  # noqa: BLE001
                log_exception()
                self._json({"ok": False, "error": msg("Unexpected error: {e}", e=e)}, 500)

        def _binary(self, op, params):
            if op.needs_project and not app.ready:
                return self._json({"ok": False, "error": msg("No project is open.")}, 400)
            try:
                data, mime = api.call(app, op.name, params)
            except AppError as e:
                return self._send(404, str(e), "text/plain")
            except Exception:  # noqa: BLE001
                log_exception()
                return self._send(500, "chyba", "text/plain")
            return self._send(200, data, mime, cache=True)

        # ---------------------------------------------------------- GET
        def do_GET(self):
            if not self._host_ok():
                return self._send(403, "forbidden", "text/plain")
            url = urlparse(self.path)
            q = parse_qs(url.query)
            sess.last_ping = time.time()
            sess.bye_at = None
            p = url.path
            if p in ("/", "/index.html"):
                return self._static("index.html")
            if p in ("/styles.css", "/crest.svg", "/favicon.svg", "/aside.png", "/selftest.js") or re.fullmatch(r"/js/[\w.-]+\.js", p):
                return self._static(p[1:])
            if not p.startswith("/api/"):
                return self._send(404, "not found", "text/plain")
            r = p[5:]
            if r == "focus":                   # with ?after=N&wait=1 it answers as soon as the focus changes (or after 20 s)
                after = q1(q, "after")
                if q1(q, "wait") == "1" and after.lstrip("-").isdigit():
                    with sess.focus_cv:
                        sess.focus_cv.wait_for(lambda: sess.focus["seq"] > int(after), timeout=20)
                return self._json({"ok": True, **sess.focus})
            if r == "ping":
                wid = q1(q, "w")
                if wid:
                    sess.windows[wid[:40]] = time.time()
                    if q1(q, "k") == "1":
                        sess.companions[wid[:40]] = time.time()
                now = time.time()
                return self._json({"ok": True, "companions": sum(1 for t in sess.companions.values() if now - t < 12)})
            op = api.ROUTES.get(("GET", r))                # every other read is an operation of Core (core/lexo/api.py)
            if op is None:
                return self._json({"ok": False, "error": msg("Unknown address.")}, 404)
            params = {k: v[0] for k, v in q.items()}
            if op.kind == "binary":                        # a font or a picture of the game
                return self._binary(op, params)
            self._run(lambda: api.call(app, op.name, params))

        # ---------------------------------------------------------- POST
        def do_POST(self):
            if not self._host_ok():
                self._drain()
                return self._send(403, "forbidden", "text/plain")
            url = urlparse(self.path)
            if url.path == "/api/bye":      # sendBeacon cannot set headers
                qs = parse_qs(url.query)
                if qs.get("t", [""])[0] == sess.token:
                    sess.windows.pop(qs.get("w", [""])[0][:40], None)
                    sess.companions.pop(qs.get("w", [""])[0][:40], None)
                    now = time.time()
                    for k in [k for k, t in sess.windows.items() if now - t > 30]:      # windows that stopped pinging
                        sess.windows.pop(k, None)
                    if not sess.windows:
                        sess.bye_at = now
                return self._send(200, "ok", "text/plain")
            if self.headers.get("X-Token") != sess.token:
                self._drain()
                return self._json({"ok": False, "error": msg("Forbidden (invalid token).")}, 403)
            try:
                length = int(self.headers.get("Content-Length", 0))
                if length > MAX_BODY:
                    return self._json({"ok": False, "error": msg("The request is too large.")}, 413)
                body = json.loads(self.rfile.read(length).decode("utf-8") or "{}")
            except (ValueError, UnicodeDecodeError):
                return self._json({"ok": False, "error": msg("Invalid request.")}, 400)
            sess.last_ping = time.time()
            r = url.path[5:] if url.path.startswith("/api/") else ""
            d = body if isinstance(body, dict) else {}

            def project_pick():
                if sess.pick_folder is None:
                    raise AppError(msg("Choosing a folder is not available in this mode – enter the path by hand."))
                chosen = sess.pick_folder()
                if not chosen:
                    return {"ok": True, "cancelled": True}
                ok, problem = check_project_dir(chosen)
                if not ok:
                    raise AppError(problem)
                app.open_project(chosen)
                return {"ok": True, "cancelled": False, "path": chosen}

            def set_focus():
                line = d.get("line", 0)
                with sess.focus_cv:
                    text = d.get("text")
                    sess.focus = {"seq": sess.focus["seq"] + 1, "file": str(d.get("file", ""))[:500], "line": int(line) if str(line).isdigit() else 0,
                                  "text": text[:20000] if isinstance(text, str) else None}      # text = what is being typed in the main window
                    sess.focus_cv.notify_all()
                return {"ok": True, **sess.focus}

            def open_window():
                """One more app window with its own list / filter state, e.g. to read the scene next to the editor."""
                if sess.open_window is None:
                    return {"ok": False, "fallback": True, "error": msg("A new window cannot be opened from the application here.")}
                q = {"win": "2"}
                if str(d.get("companion", "")) == "1":
                    q["companion"] = "1"
                if str(d.get("tab", "")) in ("check", "proof", "scene", "memory", "history", "script"):
                    q["tab"] = str(d["tab"])
                if str(d.get("view", "")) in ("overview", "files", "entries", "validation", "glossary", "publish", "settings"):
                    q["view"] = str(d["view"])
                if d.get("file"):
                    q["file"] = str(d["file"])[:500]
                if str(d.get("line", "")).isdigit():
                    q["line"] = str(int(d["line"]))
                if str(d.get("status", "")) in ("all", "todo", "translated", "approved", "review", "proofed", "fix", "stale"):
                    q["status"] = str(d["status"])
                url = sess.base_url + "?" + urlencode(q)
                try:
                    sess.open_window(url)
                except Exception as e:  # noqa: BLE001
                    return {"ok": False, "fallback": True, "error": msg("The window could not be opened ({e}).", e=e)}
                return {"ok": True, "url": url}

            op = api.ROUTES.get(("POST", r))               # operations of Core (core/lexo/api.py)
            if op is not None:
                return self._run(lambda: api.call(app, op.name, d))
            session_routes = {                              # what only the application around Core can do: windows, the focus between windows, a folder dialog
                "project/pick": project_pick,
                "window/open": lambda: open_window(),
                "window/fullscreen": lambda: ({"ok": True} if sess.toggle_fullscreen and sess.toggle_fullscreen() else {"ok": False, "fallback": True}),
                "focus": lambda: set_focus(),
            }
            fn = session_routes.get(r)
            if fn is None:
                return self._json({"ok": False, "error": msg("Unknown address.")}, 404)
            if r != "project/pick" and not app.ready:
                return self._json({"ok": False, "error": msg("No project is open.")}, 400)
            self._run(fn)

    return Handler


class LocalServer(ThreadingHTTPServer):
    """The default listen backlog (5) drops connections when a browser opens many in parallel
    (page + script + style + API calls), which showed up as sporadic ERR_CONNECTION_RESET."""
    request_queue_size = 128
    daemon_threads = True


def start_server(app, pick_folder=None):
    sess = Session(app, pick_folder)
    server = LocalServer(("127.0.0.1", 0), make_handler(sess))
    sess.base_url = f"http://127.0.0.1:{server.server_address[1]}/"
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, sess, sess.base_url
