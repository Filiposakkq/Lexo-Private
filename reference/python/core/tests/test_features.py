"""Feature catalog and extension host: gated (Pro) features exist only in the Pro pack, and without it they are locked whatever else is changed."""
import json
import unittest

from helpers import TempCase

from lexo import features
from lexo.app import AppError, FeatureLocked
from lexo.extensions import API, ExtensionError, ExtensionHost


class FakeProvider:
    """What a Pro pack looks like from Core: an object that says what it offers and answers JSON calls."""
    api = API
    version = "9.9"

    def __init__(self, offers=None):
        self._offers = offers if offers is not None else {"publish.web": {"limit": None}}
        self.calls = []

    def offers(self):
        return self._offers

    def call(self, feature, method, payload):
        self.calls.append((feature, method, payload))
        if method == "fail":
            raise ExtensionError("no way")
        if method == "live":
            return {"x": object()}
        return {"echo": payload, "method": method}


class CatalogTests(unittest.TestCase):
    def test_features_outside_the_catalog_are_free(self):
        self.assertTrue(features.decide("editor.basic", ExtensionHost()).available)

    def test_without_a_pack_every_gated_feature_is_locked(self):
        st = features.status_of(ExtensionHost())
        self.assertEqual(set(st), set(features.CATALOG))
        self.assertEqual({fid: (d["available"], d["pro"]) for fid, d in st.items()}, {fid: (False, False) for fid in features.CATALOG})

    def test_the_pack_decides_what_is_available_and_with_which_limit(self):
        host = ExtensionHost()
        host.register(FakeProvider({"mt.batch": {"limit": 20}, "publish.web": {"limit": None}}))
        st = features.status_of(host)
        self.assertEqual((st["mt.batch"]["available"], st["mt.batch"]["limit"]), (True, 20))
        self.assertEqual((st["publish.web"]["available"], st["publish.web"]["limit"]), (True, None))
        self.assertFalse(st["replace.regex"]["available"])


class ExtensionHostTests(unittest.TestCase):
    def test_without_a_pack_a_feature_is_not_available(self):
        host = ExtensionHost()
        self.assertFalse(features.decide("publish.web", host).available)
        with self.assertRaises(ExtensionError):
            host.call("publish.web", "plan", {})

    def test_a_provider_offers_features_and_limits(self):
        host = ExtensionHost()
        host.register(FakeProvider({"publish.web": {"limit": None}, "mt.batch": {"limit": 5}}))
        self.assertEqual((host.decide("publish.web").available, host.decide("publish.web").limit), (True, None))
        self.assertEqual(host.decide("mt.batch").limit, 5)
        self.assertFalse(host.decide("replace.regex").available)
        self.assertEqual(host.versions(), ["9.9"])

    def test_calls_carry_plain_json_only(self):
        host = ExtensionHost()
        p = host.register(FakeProvider())
        out = host.call("publish.web", "plan", {"a": [1, 2]})
        self.assertEqual(out["echo"], {"a": [1, 2]})
        json.dumps(out)
        with self.assertRaises(ExtensionError):
            host.call("publish.web", "plan", {"a": object()})                  # a live object cannot be sent
        with self.assertRaises(ExtensionError):
            host.call("publish.web", "live", {})                               # nor can the provider return one
        self.assertEqual([c[1] for c in p.calls], ["plan", "live"])             # the request with a live object never reached the provider

    def test_a_pack_with_another_interface_version_is_refused(self):
        class Old(FakeProvider):
            api = API + 1
        with self.assertRaises(ExtensionError):
            ExtensionHost().register(Old())

    def test_a_missing_pack_is_normal_and_a_broken_or_untrusted_one_is_only_reported(self):
        host = ExtensionHost()
        self.assertIsNone(host.load_installed("no_such_pack_module"))
        self.assertEqual((host.present, host.problems), (False, []))
        host = ExtensionHost(trust=lambda mod: False)
        self.assertIsNone(host.load_installed("json"))                         # any importable module stands in for a pack
        self.assertTrue(host.problems and not host.present)
        host = ExtensionHost()
        self.assertIsNone(host.load_installed("json"))                         # no provider() in it: reported, not fatal
        self.assertTrue(host.problems)


class AppGateTests(TempCase):
    def setUp(self):
        super().setUp()
        from helpers import dialog
        self.write("big.rpy", "".join(dialog(f"id_{n}", "e", f"Line number {n}", line=10 + n) for n in range(25)) + dialog("t1", "e", "Hello", "Hello Line", 99))
        self.app = self.new_app()
        self.app.ext = ExtensionHost()                          # a Free installation: no pack

    def test_every_gated_feature_is_locked_without_the_pack_in_every_place_it_is_used(self):
        locked = {
            "publish.web": lambda: self.app.publish_plan(),
            "replace.regex": lambda: self.app.replace_preview("a.*", "b", mode="regex"),
            "mt.batch": lambda: self.app.mt_targets("big.rpy"),
            "fixes.safe": lambda: self.app.fix_preview(kinds=["spaces"]),
            "bulk.categorize": lambda: self.app.assign_suggest(),
        }
        self.assertEqual(set(locked), set(features.CATALOG) - {"carry.over", "memory.fuzzy", "mt.profiles", "style.check", "release.build"})   # those five live only in the Rust Core and the pack
        for fid, call in locked.items():
            with self.assertRaises(FeatureLocked, msg=fid) as cm:
                call()
            self.assertEqual(cm.exception.feature, fid)
            self.assertIsInstance(cm.exception, AppError)         # the server already turns AppError into a message

    def test_the_free_halves_of_those_features_work_without_the_pack(self):
        self.assertTrue(self.app.replace_preview("Line", "Row", mode="plain")["items"])        # plain find & replace is free
        self.assertTrue(self.app.features_status()["ok"])

    def test_a_pack_that_offers_a_feature_makes_it_available(self):
        self.app.ext.register(FakeProvider())
        st = self.app.features_status()
        self.assertTrue(st["features"]["publish.web"]["available"])
        self.assertEqual((st["tier"], st["pack"]), ("pro", ["9.9"]))


if __name__ == "__main__":
    unittest.main()
