"""Writes the tiny fonts of `core/golden/fonts/` (inputs of the golden cases of `fontmetrics`). They hold only the four tables the reader looks at
(`head`, `hhea`, `hmtx`, `cmap`), no outlines, so they are a few hundred bytes. Run it only to recreate them: the files are committed and are the inputs.

    py core/tests/make_golden_fonts.py
"""
import struct
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "golden" / "fonts"


def head(upm):
    return b"\x00" * 18 + struct.pack(">H", upm) + b"\x00" * 34            # 54 bytes; the units per em are at offset 18


def hhea(n_h):
    return b"\x00" * 34 + struct.pack(">H", n_h)                          # 36 bytes; numberOfHMetrics is at offset 34


def hmtx(advances):
    return b"".join(struct.pack(">Hh", a, 0) for a in advances)


def cmap_format4(segments, glyph_ids=()):
    """segments: [(start, end, delta, range_offset)]; `glyph_ids` follow the offsets (for the segments that use them)."""
    n = len(segments)
    ends = b"".join(struct.pack(">H", e) for _s, e, _d, _r in segments)
    starts = b"".join(struct.pack(">H", s) for s, _e, _d, _r in segments)
    deltas = b"".join(struct.pack(">h", d) for _s, _e, d, _r in segments)
    ranges = b"".join(struct.pack(">H", r) for _s, _e, _d, r in segments)
    gids = b"".join(struct.pack(">H", g) for g in glyph_ids)
    body = ends + struct.pack(">H", 0) + starts + deltas + ranges + gids
    return struct.pack(">HHHHHHH", 4, 14 + len(body), 0, n * 2, 0, 0, 0) + body


def cmap_format12(groups):
    data = struct.pack(">HHIII", 12, 0, 16 + 12 * len(groups), 0, len(groups)) + b"".join(struct.pack(">III", s, e, g) for s, e, g in groups)
    return data


def cmap(subtables):
    """subtables: [(platform, encoding, bytes)]"""
    head_len = 4 + 8 * len(subtables)
    out, blobs, off = struct.pack(">HH", 0, len(subtables)), b"", head_len
    for plat, enc, blob in subtables:
        out += struct.pack(">HHI", plat, enc, off)
        blobs += blob
        off += len(blob)
    return out + blobs


def font(tables, version=b"\x00\x01\x00\x00"):
    n = len(tables)
    out, off, blobs = version + struct.pack(">HHHH", n, 0, 0, 0), 12 + 16 * n, b""
    for tag, blob in tables.items():
        out += tag.encode("latin-1") + struct.pack(">III", 0, off + len(blobs), len(blob))
        blobs += blob
    return out + blobs


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    adv = [500, 600, 700, 650, 250]                                       # glyphs: .notdef, A, B, č, space
    # format 4: one segment with delta, one with a glyph id array (range offset), the closing 0xFFFF segment
    f4 = cmap_format4([(0x20, 0x20, 4 - 0x20, 0), (0x41, 0x42, 1 - 0x41, 0), (0x10D, 0x10D, 0, 4), (0xFFFF, 0xFFFF, 1, 0)], glyph_ids=[3])
    (OUT / "format4.ttf").write_bytes(font({"head": head(1000), "hhea": hhea(5), "hmtx": hmtx(adv), "cmap": cmap([(3, 1, f4)])}))
    # format 12 chosen over a format 4 table of the same font; fewer metrics than glyphs (the last advance is shared); a code point past the preview limit
    f4_other = cmap_format4([(0x58, 0x58, 1 - 0x58, 0), (0xFFFF, 0xFFFF, 1, 0)])
    f12 = cmap_format12([(0x41, 0x43, 1), (0x20, 0x20, 4), (0x1F600, 0x1F600, 2)])
    (OUT / "format12.ttf").write_bytes(font({"head": head(2048), "hhea": hhea(3), "hmtx": hmtx(adv[:3]), "cmap": cmap([(3, 1, f4_other), (3, 10, f12)])}))
    (OUT / "no_hmtx.ttf").write_bytes(font({"head": head(1000), "hhea": hhea(5), "cmap": cmap([(3, 1, f4)])}))
    (OUT / "bad_upm.ttf").write_bytes(font({"head": head(8), "hhea": hhea(5), "hmtx": hmtx(adv), "cmap": cmap([(3, 1, f4)])}))
    (OUT / "truncated.ttf").write_bytes((OUT / "format4.ttf").read_bytes()[:100])
    (OUT / "not_a_font.bin").write_bytes(b"this is not a font file at all\n")


if __name__ == "__main__":
    main()
