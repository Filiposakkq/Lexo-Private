"""The boundary between the application and the Pro pack: the interface texts of the pack reach the browser, and neither Core nor Free ever imports the pack."""
import http.client
import json
import re
import unittest
from pathlib import Path

from helpers import TempCase

from lexo.extensions import API, ExtensionHost
from lexo_free import server

ROOT = Path(__file__).resolve().parents[2]


class PackProvider:
    api = API
    version = "1"

    def offers(self):
        return {"publish.web": {"limit": None}}

    def call(self, feature, method, payload):
        return {}

    def ui_strings(self, code):
        return {"Only the pack says this": "Jen balíček to říká"} if code == "cs" else {}


class PackTextsTests(TempCase):
    def test_the_interface_texts_of_the_pack_are_handed_out_with_those_of_core(self):
        self.basic_project()
        app = self.new_app()
        app.ext = ExtensionHost()
        app.ext.register(PackProvider())
        srv, sess, _ = server.start_server(app)
        self.addCleanup(srv.shutdown)

        def get(path):
            c = http.client.HTTPConnection("127.0.0.1", srv.server_address[1], timeout=10)
            c.request("GET", path, headers={"X-Token": sess.token})
            return json.loads(c.getresponse().read())
        cs = get("/api/ui-lang?code=cs")["strings"]
        self.assertEqual(cs["Only the pack says this"], "Jen balíček to říká")
        self.assertIn("Overview", cs)                                          # Core's own texts are still there
        self.assertNotIn("Only the pack says this", get("/api/ui-lang?code=en")["strings"])


class NoProInThePublicCodeTests(unittest.TestCase):
    def test_core_and_free_never_import_the_pro_pack(self):
        found = []
        for folder in (ROOT / "core" / "lexo", ROOT / "free" / "lexo_free"):
            for f in sorted(folder.glob("*.py")):
                for n, line in enumerate(f.read_text(encoding="utf-8").splitlines(), 1):
                    if re.match(r"\s*(import|from)\s+lexo_pro\b", line):
                        found.append(f"{f.name}:{n}")
        self.assertEqual(found, [], "the Pro pack is found by its name at run time (extensions.py), never imported")

    def test_the_code_of_the_pro_features_is_not_in_the_public_packages(self):
        self.assertFalse((ROOT / "core" / "lexo" / "publish.py").exists())        # publishing is in reference/python/pro/lexo_pro, not in the public package
        gone = ("def fixed_line", "def fix_spaces", "def fix_typos", "def plan_text_fixes", "def chain_fixes",       # safe fixes
                "def _signals", "SHARED_STEMS",                                                                        # suggesting groups for files
                "m.expand(replace)", "re.compile(find, flags | re.UNICODE)")                                            # regex replace
        found = []
        for folder in (ROOT / "core" / "lexo", ROOT / "free" / "lexo_free"):
            for f in sorted(folder.glob("*.py")):
                text = f.read_text(encoding="utf-8")
                found += [f"{f.name}: {g}" for g in gone if g in text]
        self.assertEqual(found, [], "the code of a Pro feature was found in the public packages")


if __name__ == "__main__":
    unittest.main()
