"""The browser UI against the interface of Core: it only calls operations that exist, and the server serves each read operation the way a direct call answers."""
import http.client
import json
import re
import unittest
from pathlib import Path
from urllib.parse import urlencode

import contract_tool as ct
from helpers import TempCase

from lexo import api
from lexo_free import server

UI_JS = Path(__file__).resolve().parent.parent / "lexo_free" / "ui" / "js"
# what only the application around Core serves (windows, the focus between windows, a folder dialog, the keep-alive pings, the licence of the Pro pack)
SESSION = {("GET", "focus"), ("GET", "ping"), ("POST", "focus"), ("POST", "window/open"), ("POST", "window/fullscreen"), ("POST", "project/pick"), ("POST", "bye"),
           ("GET", "licence"), ("POST", "licence/activate"), ("POST", "licence/refresh"), ("POST", "licence/deactivate"),
           ("POST", "licence/devices"), ("POST", "licence/release"), ("POST", "licence/rename"),
           ("POST", "licence/recover"), ("POST", "licence/recover-confirm")}
CALL = re.compile(r"""(?<![.\w])(get|post)\(\s*['"]([\w/.-]+)['"]""")          # get('x') / post('x'), not params.get('x')
URL = re.compile(r"""['"`]/api/([\w/.-]+)""")


class UiCallsTests(unittest.TestCase):
    def test_the_ui_only_calls_operations_that_exist(self):
        unknown = []
        for f in sorted(UI_JS.glob("*.js")):
            text = f.read_text(encoding="utf-8")
            for kind, name in CALL.findall(text):
                if (kind.upper(), name) not in api.ROUTES and (kind.upper(), name) not in SESSION:
                    unknown.append(f"{f.name}: {kind}('{name}')")
            for name in URL.findall(text):
                if not any((m, name) in api.ROUTES or (m, name) in SESSION for m in ("GET", "POST")):
                    unknown.append(f"{f.name}: /api/{name}")
        self.assertEqual(unknown, [], "the UI calls an address that Core's table (core/lexo/api.py) does not have")


class ServedLikeADirectCallTests(TempCase):
    def test_every_read_operation_is_served_with_the_keys_of_a_direct_call(self):
        app, cleanup = ct.open_fixture()
        self.addCleanup(cleanup)
        srv, sess, _ = server.start_server(app)
        self.addCleanup(srv.shutdown)

        def http_call(op):
            c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=20)
            if op.method == "GET":
                c.request("GET", f"/api/{op.path}?" + urlencode({k: ("1" if v is True else "0" if v is False else v) for k, v in op.example.items() if not isinstance(v, (list, dict))}),
                          headers={"X-Token": sess.token})
            else:
                c.request("POST", f"/api/{op.path}", json.dumps(op.example), {"X-Token": sess.token, "Content-Type": "application/json"})
            r = c.getresponse()
            return r.status, json.loads(r.read())
        checked = 0
        for op in sorted(api.OPS.values(), key=lambda o: o.name):
            if op.example is None or op.effect != "read" or op.kind != "json":
                continue
            status, body = http_call(op)
            direct = ct.plain(api.call(app, op.name, op.example))
            self.assertEqual((status, body.get("ok")), (200, True), f"{op.name}: {body}")
            self.assertEqual(sorted(body), sorted(direct), op.name)
            checked += 1
        self.assertGreater(checked, 30)

    def test_an_unknown_address_and_the_wrong_method_are_a_404(self):
        app, cleanup = ct.open_fixture()
        self.addCleanup(cleanup)
        srv, sess, _ = server.start_server(app)
        self.addCleanup(srv.shutdown)
        c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=10)
        c.request("GET", "/api/entry/save?file=a&line=1", headers={"X-Token": sess.token})        # entry/save is a POST
        self.assertEqual(c.getresponse().status, 404)


if __name__ == "__main__":
    unittest.main()
