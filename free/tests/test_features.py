"""Feature registry: gated (Pro) features, free limits, and that the development default changes nothing."""
import unittest

from helpers import TempCase, dialog

from lexo import features
from lexo.app import AppError, FeatureLocked


class EntitlementTests(unittest.TestCase):
    def test_pro_unlocks_everything(self):
        for fid in features.CATALOG:
            d = features.PRO.decide(fid)
            self.assertEqual((d.available, d.limit), (True, None))

    def test_free_limits_or_locks(self):
        self.assertEqual(features.FREE.decide("mt.batch").limit, 20)
        self.assertFalse(features.FREE.decide("replace.regex").available)
        self.assertFalse(features.FREE.decide("publish.web").available)

    def test_features_outside_the_catalog_are_free(self):
        self.assertTrue(features.FREE.decide("editor.basic").available)

    def test_status_lists_every_gated_feature(self):
        st = features.FREE.status()
        self.assertEqual(set(st), set(features.CATALOG))
        self.assertEqual(st["mt.batch"]["limit"], 20)


class AppGateTests(TempCase):
    def setUp(self):
        super().setUp()
        text = "".join(dialog(f"id_{n}", "e", f"Line number {n}", line=10 + n) for n in range(25))
        self.write("big.rpy", text)
        self.app = self.new_app()

    def test_dev_default_is_unlimited(self):
        r = self.app.mt_targets()
        self.assertEqual((len(r["items"]), r["total"]), (25, 25))      # 25 untranslated lines in big.rpy
        self.assertNotIn("limited", r)
        self.assertTrue(self.app.features_status()["features"]["publish.web"]["available"])

    def test_free_machine_translation_targets_are_cut_and_the_total_is_told(self):
        self.app.entitlements = features.FREE
        r = self.app.mt_targets()
        self.assertEqual(len(r["items"]), 20)
        self.assertEqual(r["total"], 25)
        self.assertEqual(r["limited"], {"feature": "mt.batch", "limit": 20, "total": r["total"]})

    def test_free_cannot_publish_or_use_regex(self):
        self.app.entitlements = features.FREE
        with self.assertRaises(FeatureLocked) as cm:
            self.app.publish_plan()
        self.assertEqual(cm.exception.feature, "publish.web")
        self.assertIsInstance(cm.exception, AppError)         # the server already turns AppError into a message
        with self.assertRaises(FeatureLocked):
            self.app.replace_preview("a.*", "b", mode="regex")
        self.app.replace_preview("Line", "Řádek", mode="plain")  # plain find & replace stays free

    def test_free_fix_preview_is_capped(self):
        self.app.entitlements = features.FREE
        r = self.app.fix_preview(kinds=["spaces"])
        self.assertLessEqual(len(r["items"]), 20)


if __name__ == "__main__":
    unittest.main()
