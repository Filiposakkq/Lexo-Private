"""The UI is ES modules with one entry point (js/99-boot.js). These checks catch a forgotten export, a missing file or an orphan."""
import re
import unittest
from pathlib import Path

UI = Path(__file__).resolve().parent.parent / "lexo_free" / "ui"

IMPORT_RE = re.compile(r"^import \{([^}]*)\} from '\./([^']+)';", re.M)
SIDE_RE = re.compile(r"^import '\./([^']+)';", re.M)
EXPORT_RE = re.compile(r"^export (?:async )?(?:function\*?\s+|const\s+|let\s+|var\s+|class\s+)([\w$]+)", re.M)
EXPORT_DESTRUCT_RE = re.compile(r"^export (?:const|let|var)\s+[\[{]([^=]*)[\]}]\s*=", re.M)


def exported(text):
    names = set(EXPORT_RE.findall(text))
    for group in EXPORT_DESTRUCT_RE.findall(text):
        names.update(p.strip().split(":")[-1].split("=")[0].strip() for p in group.split(","))
    return names


class UiModuleTests(unittest.TestCase):
    def setUp(self):
        self.js = {f.name: f.read_text(encoding="utf-8") for f in sorted((UI / "js").glob("*.js"))}
        self.assertTrue(self.js)

    def test_index_loads_only_the_entry_module(self):
        html = (UI / "index.html").read_text(encoding="utf-8")
        self.assertEqual(re.findall(r'<script[^>]*src="([^"]+)"', html), ["/js/99-boot.js"])
        self.assertIn('type="module"', html)

    def test_imports_point_to_existing_files_and_names_are_exported(self):
        exports = {n: exported(t) for n, t in self.js.items()}
        for n, t in self.js.items():
            for names, src in IMPORT_RE.findall(t):
                self.assertIn(src, self.js, f"{n} imports from a missing file {src}")
                for name in (x.strip().split(" as ")[0] for x in names.split(",") if x.strip()):
                    self.assertIn(name, exports[src], f"{n} imports {name}, which {src} does not export")

    def test_every_file_is_reachable_from_the_entry(self):
        graph = {n: [src for _names, src in IMPORT_RE.findall(t)] + SIDE_RE.findall(t) for n, t in self.js.items()}
        seen, stack = set(), ["99-boot.js"]
        while stack:
            cur = stack.pop()
            if cur not in seen:
                seen.add(cur)
                stack.extend(graph[cur])
        self.assertEqual(sorted(set(self.js) - seen), [])

    def test_the_entry_hands_the_selftest_names_to_the_global_scope(self):
        boot = self.js["99-boot.js"]
        self.assertIn("Object.assign(window,", boot)
        self.assertIn("selftest.js", boot)


if __name__ == "__main__":
    unittest.main()
