> **This is the Python reference implementation.** The product is the Rust core in [`../rust/`](../../../rust/); this folder is what the port was checked against, the generator of the golden data (`golden/`, `tests/golden_tool.py`) and the interface contract (`contract/`). `lexo/packs/` (the language packs) is used by the product. Map of the repository: [`../README.md`](../../../README.md), plan to tidy it: [`../docs/REPO-CLEANUP.md`](../../../docs/REPO-CLEANUP.md).

# Core (`reference/python/core/lexo`)

The library behind Lexo: reading and writing `.rpy`, `.rpa`, lint, font metrics, the project model, settings, snapshots, history, language packs, checks, the
feature registry. **No UI, no HTTP server, no window**: `App` (`lexo.app`) is assembled from the parts `app_*.py` and everything the application does is a method of it.
Free (`reference/python/free/lexo_free`) is the first client; it only adds the JSON server, the command line and the browser UI.

```python
from lexo.app import App
from lexo.config import Config

app = App(Config().load())
app.open_project(r"D:\games\MyGame\game\tl\czech")
print(app.overview()["totals"])          # the same data the UI shows
```

Tests: `py reference/python/run_tests.py core` from the repository root (`reference/python/core/tests`; Free is not on the import path of this suite, which is how it proves that Core does not need it).
Layout of the repository: see `../../../docs/ARCHITECTURE.md`.
