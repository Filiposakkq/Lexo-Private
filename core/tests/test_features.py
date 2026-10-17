"""Feature registry and extension host: gated (Pro) features, what the host offers, and that a feature without a provider cannot be used."""
import json
import unittest

from helpers import TempCase, dialog

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


class EntitlementTests(unittest.TestCase):
    def test_pro_unlocks_everything(self):
        for fid in features.CATALOG:
            d = features.PRO.decide(fid)
            self.assertEqual((d.available, d.limit), (True, None))

    def test_free_limits_or_locks(self):
        self.assertEqual(features.FREE.decide("fixes.safe").limit, 20)
        self.assertFalse(features.FREE.decide("replace.regex").available)

    def test_features_outside_the_catalog_are_free(self):
        self.assertTrue(features.FREE.decide("editor.basic").available)

    def test_status_lists_every_gated_feature(self):
        st = features.status_of(features.FREE, ExtensionHost())
        self.assertEqual(set(st), set(features.CATALOG))
        self.assertEqual(st["fixes.safe"]["limit"], 20)
        self.assertEqual((st["publish.web"]["available"], st["publish.web"]["pro"]), (False, False))
        self.assertEqual((st["publish.web"]["extracted"], st["fixes.safe"]["extracted"]), (True, False))      # which features are already apart from Core


class ExtensionHostTests(unittest.TestCase):
    def test_without_a_pack_an_extracted_feature_is_not_available_whatever_the_flags_say(self):
        host = ExtensionHost()
        self.assertFalse(features.decide("publish.web", features.PRO, host).available)     # the development flag of Core does not unlock it
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
        text = "".join(dialog(f"id_{n}", "e", f"Line number {n}", line=10 + n) for n in range(25))
        self.write("big.rpy", text)
        self.app = self.new_app()

    def test_publishing_needs_the_pro_pack_even_in_development(self):
        self.app.ext = ExtensionHost()                          # a Free installation: no pack
        st = self.app.features_status()
        self.assertFalse(st["features"]["publish.web"]["available"])
        with self.assertRaises(FeatureLocked) as cm:
            self.app.publish_plan()
        self.assertEqual(cm.exception.feature, "publish.web")
        self.assertIsInstance(cm.exception, AppError)         # the server already turns AppError into a message

    def test_a_pack_that_offers_publishing_makes_it_available(self):
        self.app.ext = ExtensionHost()
        self.app.ext.register(FakeProvider())
        st = self.app.features_status()
        self.assertTrue(st["features"]["publish.web"]["available"])
        self.assertEqual((st["tier"], st["pack"]), ("pro", ["9.9"]))

    def test_free_cannot_use_regex_replace(self):
        self.app.entitlements = features.FREE
        with self.assertRaises(FeatureLocked):
            self.app.replace_preview("a.*", "b", mode="regex")
        self.app.replace_preview("Line", "Řádek", mode="plain")  # plain find & replace stays free

    def test_free_fix_preview_is_capped(self):
        self.app.entitlements = features.FREE
        r = self.app.fix_preview(kinds=["spaces"])
        self.assertLessEqual(len(r["items"]), 20)


if __name__ == "__main__":
    unittest.main()
