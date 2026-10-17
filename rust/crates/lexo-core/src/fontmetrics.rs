//! Port of `core/lexo/fontmetrics.py`: advance widths of a font (TrueType / OpenType file), read from `cmap` and `hmtx` only.
//!
//! Ren'Py draws text with FreeType, which takes the advance of a glyph from `hmtx`. A browser may take it from the CFF charstring instead, so the scene preview
//! places every character by the `hmtx` advance. This reads the two tables without any dependency and never trusts the file: anything out of range gives `None`.
use std::collections::BTreeMap;

/// The preview only needs Latin, punctuation and the like.
const MAX_CP: u32 = 0x3000;
const MAX_GLYPHS: u32 = 70000;

fn u16_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u16::from_be_bytes(b.get(o..o.checked_add(2)?)?.try_into().ok()?) as u32)
}

fn i16_at(b: &[u8], o: usize) -> Option<i32> {
    Some(i16::from_be_bytes(b.get(o..o.checked_add(2)?)?.try_into().ok()?) as i32)
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(o..o.checked_add(4)?)?.try_into().ok()?))
}

/// `{tag: (offset, length)}`
fn tables(data: &[u8]) -> Option<BTreeMap<String, (usize, usize)>> {
    if data.len() < 12 || ![&b"\x00\x01\x00\x00"[..], b"OTTO", b"true"].contains(&&data[..4]) {
        return None;
    }
    let mut out = BTreeMap::new();
    for i in 0..u16_at(data, 4)? as usize {
        let o = 12 + i * 16;
        if o + 16 > data.len() {
            return None;
        }
        let tag: String = data[o..o + 4].iter().map(|c| *c as char).collect();
        let (off, ln) = (u32_at(data, o + 8)? as usize, u32_at(data, o + 12)? as usize);
        if off + ln > data.len() {
            return None;
        }
        out.insert(tag, (off, ln));
    }
    Some(out)
}

/// `{codepoint: glyph id}` from the best Unicode subtable (format 12 or 4).
fn cmap(data: &[u8], off: usize) -> Option<BTreeMap<u32, u32>> {
    let n = u16_at(data, off + 2)?;
    let mut best: Option<(u32, usize, u32)> = None;
    for i in 0..n as usize {
        let rec = off + 4 + i * 8;
        let (plat, enc, sub) = (u16_at(data, rec)?, u16_at(data, rec + 2)?, u32_at(data, rec + 4)? as usize);
        if !(plat == 0 || (plat == 3 && (enc == 1 || enc == 10))) {
            continue;
        }
        let fmt = u16_at(data, off + sub)?;
        let rank = (if fmt == 12 {
            3
        } else if fmt == 4 {
            2
        } else {
            0
        }) + u32::from((plat, enc) == (3, 10));
        if (fmt == 4 || fmt == 12) && best.is_none_or(|b| rank > b.0) {
            best = Some((rank, off + sub, fmt));
        }
    }
    let Some((_, base, fmt)) = best else { return Some(BTreeMap::new()) };
    let mut cm = BTreeMap::new();
    if fmt == 12 {
        for g in 0..u32_at(data, base + 12)? as usize {
            let at = base + 16 + g * 12;
            let (start, end, gid) = (u32_at(data, at)?, u32_at(data, at + 4)?, u32_at(data, at + 8)?);
            for cp in start..=end.min(MAX_CP - 1) {
                cm.insert(cp, gid + (cp - start));
            }
        }
        return Some(cm);
    }
    let segs = u16_at(data, base + 6)? as usize / 2;
    let (ends, starts) = (base + 14, base + 16 + segs * 2);
    let (deltas, ranges) = (starts + segs * 2, starts + segs * 4);
    for s in 0..segs {
        let (end, start) = (u16_at(data, ends + s * 2)?, u16_at(data, starts + s * 2)?);
        let delta = i16_at(data, deltas + s * 2)?;
        let rng = u16_at(data, ranges + s * 2)? as usize;
        for cp in start..=end.min(MAX_CP - 1) {
            if cp == 0xFFFF {
                continue;
            }
            let gid = if rng == 0 {
                (cp as i32 + delta) as u32 & 0xFFFF
            } else {
                let o = ranges + s * 2 + rng + (cp - start) as usize * 2;
                if o + 2 > data.len() {
                    continue;
                }
                let gid = u16_at(data, o)?;
                if gid != 0 {
                    (gid as i32 + delta) as u32 & 0xFFFF
                } else {
                    gid
                }
            };
            if gid != 0 {
                cm.insert(cp, gid);
            }
        }
    }
    Some(cm)
}

#[derive(Debug, PartialEq)]
pub struct Advances {
    /// Units per em.
    pub upm: u32,
    /// Advance in font units for each code point.
    pub adv: BTreeMap<u32, u32>,
}

/// The advances of a font, or `None` when the file is not a usable font.
pub fn advances(data: &[u8]) -> Option<Advances> {
    let t = tables(data)?;
    let get = |k: &str| t.get(k).copied();
    let (head, hhea, hmtx, cm_table) = (get("head")?, get("hhea")?, get("hmtx")?, get("cmap")?);
    let upm = u16_at(data, head.0 + 18)?;
    let n_h = u16_at(data, hhea.0 + 34)?;
    if !(16..=16384).contains(&upm) || n_h == 0 || n_h > MAX_GLYPHS || n_h as usize * 4 > hmtx.1 {
        return None;
    }
    let cm = cmap(data, cm_table.0)?;
    let mut adv = BTreeMap::new();
    for (cp, gid) in cm {
        let i = gid.min(n_h - 1) as usize;
        adv.insert(cp, u16_at(data, hmtx.0 + i * 4)?);
    }
    if adv.is_empty() {
        None
    } else {
        Some(Advances { upm, adv })
    }
}
