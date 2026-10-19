//! Port of `reference/python/core/lexo/rpa.py`: read-only access to Ren'Py archives (`.rpa`, versions 2 and 3).
//!
//! An archive is a header line (`RPA-3.0 <index offset hex> <key hex>`), the file data, and at the offset a zlib compressed pickle of
//! `{name: [(offset, length[, prefix])]}`. The pickle comes from a file on disk, so it is read with the restricted reader of `pickle.rs`.
//! Nothing here ever writes.
//!
//! Known differences from Python: an entry whose name is not text is skipped (Python would use its `str()`), an offset or length beyond 64 bits is skipped.
use crate::messages::{msg, Message};
use crate::pickle::{self, Obj};
use crate::util::py_split_ws;
use flate2::read::ZlibDecoder;
use serde_json::json;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// The decompressed index of even a huge game is a few MB.
pub const MAX_INDEX: u64 = 128 * 1024 * 1024;

#[derive(Debug, PartialEq)]
pub struct RpaError(pub Message);

impl std::fmt::Display for RpaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.text())
    }
}

/// Where one file of an archive lies: `prefix` bytes come first, then the data.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub offset: u64,
    pub length: u64,
    pub prefix: Vec<u8>,
}

fn cannot_read(reason: impl ToString) -> RpaError {
    RpaError(msg("the archive cannot be read: {reason}", &[("reason", json!(reason.to_string()))]))
}

/// Python's `int(text, 16)`: optional sign and `0x`, underscores between digits, surrounding white space.
fn int_hex(text: &str) -> Result<i128, String> {
    let bad = || format!("invalid literal for int() with base 16: '{text}'");
    let t = text.trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let t = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t);
    if t.is_empty() || t.starts_with('_') || t.ends_with('_') || t.contains("__") {
        return Err(bad());
    }
    let digits: String = t.chars().filter(|c| *c != '_').collect();
    let v = i128::from_str_radix(&digits, 16).map_err(|_| bad())?;
    Ok(if neg { -v } else { v })
}

/// `int(obj)` for the values an index can hold; `None` when Python would raise.
fn to_int(o: &Obj) -> Option<i128> {
    match o {
        Obj::Int(i) => Some(*i),
        Obj::Bool(b) => Some(*b as i128),
        Obj::Float(f) if f.is_finite() => Some(f.trunc() as i128),
        Obj::Str(s) => s.trim().parse::<i128>().ok(),
        _ => None,
    }
}

fn seq(o: &Obj) -> Option<&[Obj]> {
    match o {
        Obj::List(v) | Obj::Tuple(v) => Some(v),
        _ => None,
    }
}

/// `(version, {name: entry})` with the names in the order of the index. Fails with `RpaError` for anything that is not a usable archive.
pub fn read_index(path: &Path) -> Result<(String, Vec<(String, Entry)>), RpaError> {
    let mut f = File::open(path).map_err(cannot_read)?;
    let mut line = Vec::new();
    BufReader::new((&mut f).take(80)).read_until(b'\n', &mut line).map_err(cannot_read)?;
    let head_text: String = line.iter().map(|c| *c as char).collect();
    let head = py_split_ws(&head_text);
    let version = match head.first() {
        Some(v) if *v == "RPA-3.0" || *v == "RPA-2.0" => v.to_string(),
        _ => return Err(RpaError(msg("not an RPA 2 / 3 archive", &[]))),
    };
    let off = int_hex(head.get(1).ok_or_else(|| cannot_read("list index out of range"))?).map_err(cannot_read)?;
    let key = if version == "RPA-3.0" { int_hex(head.get(2).ok_or_else(|| cannot_read("list index out of range"))?).map_err(cannot_read)? } else { 0 };
    let off = u64::try_from(off).map_err(|_| cannot_read("negative seek value"))?;
    f.seek(SeekFrom::Start(off)).map_err(cannot_read)?;
    let mut comp = Vec::new();
    f.read_to_end(&mut comp).map_err(cannot_read)?;

    let mut raw = Vec::new();
    match ZlibDecoder::new(&comp[..]).take(MAX_INDEX + 1).read_to_end(&mut raw) {
        Ok(_) => {}
        // like Python's decompress object, a stream that ends early is not an error by itself: what was decoded is unpickled (and fails there if it is cut)
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {}
        Err(e) => return Err(cannot_read(e)),
    }
    if raw.len() as u64 > MAX_INDEX {
        return Err(RpaError(msg("the archive index is unusually large", &[])));
    }
    let idx = pickle::load(&raw).map_err(|e| cannot_read(e.0))?;
    let Obj::Dict(pairs) = idx else { return Err(RpaError(msg("the archive index has an unexpected shape", &[]))) };

    let mut out: Vec<(String, Entry)> = Vec::new();
    let mut at: HashMap<String, usize> = HashMap::new();
    for (name, items) in &pairs {
        let Obj::Str(name) = name else { continue };
        let Some(e) = seq(items).and_then(|i| i.first()).and_then(seq) else { continue };
        let (Some(offset), Some(length)) = (e.first().and_then(to_int), e.get(1).and_then(to_int)) else { continue };
        let (offset, length) = (offset ^ key, length ^ key);
        let prefix = match e.get(2) {
            None => Vec::new(),
            Some(Obj::Bytes(b)) => b.clone(),
            Some(Obj::Str(s)) if s.chars().all(|c| (c as u32) < 256) => s.chars().map(|c| c as u8).collect(),
            Some(_) => continue,
        };
        let (Ok(offset), Ok(length)) = (u64::try_from(offset), u64::try_from(length)) else { continue };
        let name = name.replace('\\', "/");
        let entry = Entry { offset, length, prefix };
        match at.get(&name) {
            Some(i) => out[*i].1 = entry,
            None => {
                at.insert(name.clone(), out.len());
                out.push((name, entry));
            }
        }
    }
    Ok((version, out))
}

/// The bytes of one file of an archive (prefix + data), at most `limit` bytes.
pub fn read_entry(path: &Path, entry: &Entry, limit: u64) -> Result<Vec<u8>, RpaError> {
    if entry.length > limit {
        return Err(RpaError(msg("the file in the archive is too large", &[])));
    }
    let fail = |e: std::io::Error| RpaError(msg("the archive cannot be read: {reason}", &[("reason", json!(e.to_string()))]));
    let mut f = File::open(path).map_err(fail)?;
    f.seek(SeekFrom::Start(entry.offset)).map_err(fail)?;
    let want = entry.length.saturating_sub(entry.prefix.len() as u64);
    let mut data = Vec::new();
    f.take(want).read_to_end(&mut data).map_err(fail)?;
    let mut out = entry.prefix.clone();
    out.extend(data);
    Ok(out)
}

/// The default `limit` of `read_entry` in Python: 64 MB.
pub const DEFAULT_ENTRY_LIMIT: u64 = 64 * 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_hex_follows_python() {
        assert_eq!(int_hex("1f"), Ok(31));
        assert_eq!(int_hex("0X1F"), Ok(31));
        assert_eq!(int_hex("-a"), Ok(-10));
        assert_eq!(int_hex("1_0"), Ok(16));
        assert!(int_hex("").is_err());
        assert!(int_hex("1__0").is_err());
        assert!(int_hex("zz").is_err());
    }

    #[test]
    fn a_file_that_is_not_an_archive_is_refused_with_the_plain_message() {
        let dir = std::env::temp_dir().join(format!("lexo-rpa-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("x.rpa");
        std::fs::write(&p, b"hello world\n").unwrap();
        assert_eq!(read_index(&p).unwrap_err().to_string(), "not an RPA 2 / 3 archive");
        std::fs::write(&p, b"RPA-3.0 0\n").unwrap();
        assert!(read_index(&p).unwrap_err().to_string().starts_with("the archive cannot be read"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
