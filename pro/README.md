> **The Pro pack that ships is the Rust one in [`rust/`](rust/)** (sidecar program `lexo-pro-sidecar`; features: batch machine translation, safe fixes, regex replace, group suggestions, publishing, carry-over; the licence client). The Python pack described below (`lexo_pro/`, `tests/`) is the reference it was checked against. Map of the repository: [`../README.md`](../README.md).

# pro (private, never part of the public repository)

The Pro pack: the package `lexo_pro` (`reference/python/pro/lexo_pro/`) with its tests (`reference/python/pro/tests/`). It depends on Core (`lexo`); Core and Free never import it, they find it by name at run time
(`reference/python/core/lexo/extensions.py`). Today it holds the publishing of the progress data to a website repository (`publish.web`).

Why it is kept apart, how it is delivered and updated, what is done and what is not: [../docs/PRO-DESIGN.md](../docs/PRO-DESIGN.md).

Tests: `py reference/python/run_tests.py pro` from the repository root. Run the application without the pack, as a Free installation would run: `set LEXO_NO_PRO=1` and `py reference/python/free/run_lexo.py`.

The Rust version of the pack is in `pro/rust/` (its own Cargo workspace; it needs `rust/crates/lexo-core`). Tests: `cd pro/rust && cargo test`. It is checked against the Python pack with `testdata/pro-golden/pack.json` (`py reference/python/pro/tests/golden_pro.py` rewrites it).
