"""Writes the archives the gametools scenario unpacks (core/golden/data/gametools.json). They are kept in the repository; run this only to understand how they were made:

    py core/golden/gametools/make_archives.py

`scripts.rpa` is an RPA-3.0 archive (key 0x1234) with scripts, a picture, a translation inside the game and names that try to leave the target folder; `lies.rpa` has entries whose
index promises more bytes than the file has.
"""
import pickle
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
KEY = 0x1234


def build(path, entries, key=KEY):
    """entries: [(name, data bytes, prefix str, claimed length or None, claimed offset or None)]"""
    body = bytearray()
    head = b"RPA-3.0 %016x %08x\n" % (0, key)
    base = len(head)
    index = {}
    for name, data, prefix, claim_len, claim_off in entries:
        off = base + len(body)
        body += data
        length = len(data) + len(prefix) if claim_len is None else claim_len
        index[name] = [((off if claim_off is None else claim_off) ^ key, length ^ key, prefix)]
    packed = zlib.compress(pickle.dumps(index, protocol=2))
    head = b"RPA-3.0 %016x %08x\n" % (base + len(body), key)
    assert len(head) == base
    Path(path).write_bytes(head + bytes(body) + packed)


build(HERE / "scripts.rpa", [
    ("scripts/a.rpyc", b"RPYC2 compiled a\n", "", None, None),
    ("scripts/b.rpy", "label b:\n    \"Hello\"\n".encode(), "", None, None),
    ("scripts/m.rpymc", b"RPYC2 module\n", "", None, None),
    ("scripts/Shout.RPYC", b"RPYC2 upper case suffix\n", "", None, None),
    ("images/x.png", b"\x89PNG not really", "", None, None),
    ("tl/russian/script.rpy", "translate russian start_1:\n    \"x\"\n".encode(), "", None, None),
    ("prefixed.rpy", b"tail of a prefixed script\n", "# head\n", None, None),
    ("../evil.rpy", b"outside\n", "", None, None),
    ("/abs.rpy", b"absolute\n", "", None, None),
    ("C:/win.rpy", b"drive\n", "", None, None),
    ("a/../../up.rpy", b"up\n", "", None, None),
    ("back\\slash\\d.rpy", b"windows separators\n", "", None, None),
    ("./dot/./e.rpy", b"dots\n", "", None, None),
])
build(HERE / "lies.rpa", [
    ("ok.rpy", b"fine\n", "", None, None),
    ("short.rpy", b"short\n", "", 5000, None),
    ("beyond.rpy", b"beyond\n", "", None, 10 ** 6),
    ("huge.rpy", b"huge\n", "", 3 * 1024 ** 3, None),
])
