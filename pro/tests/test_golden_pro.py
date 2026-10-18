"""The golden file of the Rust pack (pro/golden/pack.json) is what the Python pack gives today: a change of the Python pack without regenerating it fails here."""
import json
import unittest

import golden_pro


class GoldenPackTests(unittest.TestCase):
    def test_pack_json_is_up_to_date(self):
        want = json.dumps(golden_pro.build(), ensure_ascii=False, indent=1) + "\n"
        self.assertTrue(golden_pro.OUT.is_file(), "py pro/tests/golden_pro.py")
        self.assertEqual(golden_pro.OUT.read_text(encoding="utf-8"), want, "pro/golden/pack.json is out of date: py pro/tests/golden_pro.py")


if __name__ == "__main__":
    unittest.main()
