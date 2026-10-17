//! A restricted reader of Python pickles, for the index of an `.rpa` archive (which comes from a file on disk).
//!
//! It builds nothing but plain values: None, booleans, numbers, strings, bytes, lists, tuples and dicts. Every opcode that could create an
//! object or call something (GLOBAL, REDUCE, BUILD, INST, OBJ, STACK_GLOBAL, EXT*, ...) is refused, like the unpickler of `rpa.py` that blocks every class.
//! The reader is iterative (a hostile file cannot overflow the stack), nesting is limited, and a length that claims more than the file holds is refused
//! before anything is allocated.
//!
//! Differences from Python (nothing in an index needs them): a memoised list or dict cannot be fetched again (they are mutable and shared, the reader
//! keeps snapshots), `encoding="latin-1"` is applied to the 8-bit string opcodes of Python 2.
use std::collections::HashMap;

const MAX_DEPTH: u16 = 100;

#[derive(Clone, Debug, PartialEq)]
pub enum Obj {
    None,
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<Obj>),
    Tuple(Vec<Obj>),
    Dict(Vec<(Obj, Obj)>),
}

#[derive(Debug, PartialEq)]
pub struct PickleError(pub String);

fn err<T>(s: impl Into<String>) -> Result<T, PickleError> {
    Err(PickleError(s.into()))
}

/// What a stack slot holds: a value (with its nesting depth) or a MARK.
#[derive(Clone)]
enum Slot {
    Mark,
    Val(Obj, u16),
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], PickleError> {
        if n > self.data.len() - self.pos {
            return err("pickle data was truncated");
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn byte(&mut self) -> Result<u8, PickleError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u64, PickleError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()) as u64)
    }
    fn u32(&mut self) -> Result<u64, PickleError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()) as u64)
    }
    fn u64(&mut self) -> Result<u64, PickleError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn len(&mut self, n: u64) -> Result<usize, PickleError> {
        usize::try_from(n).ok().filter(|n| *n <= self.data.len() - self.pos).ok_or_else(|| PickleError("pickle data was truncated".into()))
    }
    fn line(&mut self) -> Result<&'a [u8], PickleError> {
        let rest = &self.data[self.pos..];
        match rest.iter().position(|b| *b == b'\n') {
            Some(i) => {
                self.pos += i + 1;
                Ok(&rest[..i])
            }
            None => err("pickle data was truncated"),
        }
    }
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|c| *c as char).collect()
}

fn decode_long(b: &[u8]) -> Result<i128, PickleError> {
    if b.is_empty() {
        return Ok(0);
    }
    if b.len() > 16 {
        return err("integer too large");
    }
    let mut v: i128 = if b[b.len() - 1] & 0x80 != 0 { -1 } else { 0 };
    for byte in b.iter().rev() {
        v = (v << 8) | *byte as i128;
    }
    Ok(v)
}

fn hashable(o: &Obj) -> bool {
    match o {
        Obj::List(_) | Obj::Dict(_) => false,
        Obj::Tuple(t) => t.iter().all(hashable),
        _ => true,
    }
}

fn parse_text_int(b: &[u8]) -> Result<Obj, PickleError> {
    let s = latin1(b);
    let s = s.trim();
    match s {
        "00" => return Ok(Obj::Bool(false)),
        "01" => return Ok(Obj::Bool(true)),
        _ => {}
    }
    let s = s.strip_suffix('L').unwrap_or(s);
    s.parse::<i128>().map(Obj::Int).or_else(|_| err("invalid integer literal"))
}

/// Read one pickle and return the value it stops with.
pub fn load(data: &[u8]) -> Result<Obj, PickleError> {
    let mut r = Reader { data, pos: 0 };
    let mut stack: Vec<Slot> = Vec::new();
    let mut memo: HashMap<u64, (Obj, u16)> = HashMap::new();

    fn pop(stack: &mut Vec<Slot>) -> Result<(Obj, u16), PickleError> {
        match stack.pop() {
            Some(Slot::Val(o, d)) => Ok((o, d)),
            Some(Slot::Mark) => err("unexpected MARK"),
            None => err("unpickling stack underflow"),
        }
    }
    /// The values above the last MARK (the MARK is removed).
    fn pop_mark(stack: &mut Vec<Slot>) -> Result<Vec<(Obj, u16)>, PickleError> {
        let at = stack.iter().rposition(|s| matches!(s, Slot::Mark)).ok_or_else(|| PickleError("MARK missing".into()))?;
        let items = stack.split_off(at + 1);
        stack.pop();
        items.into_iter().map(|s| if let Slot::Val(o, d) = s { Ok((o, d)) } else { err("unexpected MARK") }).collect()
    }
    fn depth_of(items: &[(Obj, u16)]) -> Result<u16, PickleError> {
        let d = items.iter().map(|(_, d)| *d).max().unwrap_or(0) + 1;
        if d > MAX_DEPTH {
            return err("pickle is nested too deeply");
        }
        Ok(d)
    }
    fn memoize(memo: &mut HashMap<u64, (Obj, u16)>, key: u64, top: Option<&Slot>) -> Result<(), PickleError> {
        match top {
            Some(Slot::Val(o, d)) => {
                if memo.len() >= 10_000_000 {
                    return err("pickle memo is too large");
                }
                memo.insert(key, (o.clone(), *d));
                Ok(())
            }
            _ => err("nothing to memoize"),
        }
    }
    fn fetch(memo: &HashMap<u64, (Obj, u16)>, key: u64) -> Result<Slot, PickleError> {
        match memo.get(&key) {
            Some((Obj::List(_) | Obj::Dict(_), _)) => err("a shared list or dict is not supported"),
            Some((o, d)) => Ok(Slot::Val(o.clone(), *d)),
            None => err("memo key missing"),
        }
    }
    fn push(stack: &mut Vec<Slot>, o: Obj) {
        stack.push(Slot::Val(o, 0));
    }

    let mut next_memo: u64 = 0;
    loop {
        let op = r.byte()?;
        match op {
            0x80 => {
                let v = r.byte()?;
                if v > 5 {
                    return err("unsupported pickle protocol");
                }
            }
            0x95 => {
                r.take(8)?; // FRAME: the length of the frame, the opcodes follow as usual
            }
            b'.' => {
                return match stack.pop() {
                    Some(Slot::Val(o, _)) => Ok(o),
                    _ => err("unpickling stack underflow"),
                };
            }
            b'N' => push(&mut stack, Obj::None),
            0x88 => push(&mut stack, Obj::Bool(true)),
            0x89 => push(&mut stack, Obj::Bool(false)),
            b'I' => {
                let l = r.line()?;
                push(&mut stack, parse_text_int(l)?)
            }
            b'L' => {
                let l = r.line()?;
                push(&mut stack, parse_text_int(l)?)
            }
            b'J' => {
                let v = i32::from_le_bytes(r.take(4)?.try_into().unwrap());
                push(&mut stack, Obj::Int(v as i128))
            }
            b'K' => {
                let v = r.byte()?;
                push(&mut stack, Obj::Int(v as i128))
            }
            b'M' => {
                let v = r.u16()?;
                push(&mut stack, Obj::Int(v as i128))
            }
            0x8a => {
                let n = r.byte()? as u64;
                let n = r.len(n)?;
                let v = decode_long(r.take(n)?)?;
                push(&mut stack, Obj::Int(v))
            }
            0x8b => {
                let n = r.u32()?;
                let n = r.len(n)?;
                let v = decode_long(r.take(n)?)?;
                push(&mut stack, Obj::Int(v))
            }
            b'F' => {
                let l = r.line()?;
                let v: f64 = latin1(l).trim().parse().or_else(|_| err("invalid float"))?;
                push(&mut stack, Obj::Float(v))
            }
            b'G' => {
                let b: [u8; 8] = r.take(8)?.try_into().unwrap();
                push(&mut stack, Obj::Float(f64::from_be_bytes(b)))
            }
            b'X' => {
                let n = r.u32()?;
                let n = r.len(n)?;
                let s = String::from_utf8(r.take(n)?.to_vec()).or_else(|_| err("invalid UTF-8 in a string"))?;
                push(&mut stack, Obj::Str(s))
            }
            0x8c => {
                let n = r.byte()? as u64;
                let n = r.len(n)?;
                let s = String::from_utf8(r.take(n)?.to_vec()).or_else(|_| err("invalid UTF-8 in a string"))?;
                push(&mut stack, Obj::Str(s))
            }
            0x8d => {
                let n = r.u64()?;
                let n = r.len(n)?;
                let s = String::from_utf8(r.take(n)?.to_vec()).or_else(|_| err("invalid UTF-8 in a string"))?;
                push(&mut stack, Obj::Str(s))
            }
            b'S' => {
                // a quoted 8-bit string of Python 2: 'text' or "text" with escapes; only the plain form is read
                let l = r.line()?;
                if l.len() < 2 || l[0] != l[l.len() - 1] || !(l[0] == b'\'' || l[0] == b'"') || l.contains(&b'\\') {
                    return err("unsupported string literal");
                }
                push(&mut stack, Obj::Str(latin1(&l[1..l.len() - 1])))
            }
            b'T' => {
                let n = r.u32()?;
                let n = r.len(n)?;
                push(&mut stack, Obj::Str(latin1(r.take(n)?)))
            }
            b'U' => {
                let n = r.byte()? as u64;
                let n = r.len(n)?;
                push(&mut stack, Obj::Str(latin1(r.take(n)?)))
            }
            b'V' => {
                let l = r.line()?;
                if l.contains(&b'\\') {
                    return err("unsupported unicode literal");
                }
                let s = String::from_utf8(l.to_vec()).or_else(|_| err("invalid UTF-8 in a string"))?;
                push(&mut stack, Obj::Str(s))
            }
            b'B' => {
                let n = r.u32()?;
                let n = r.len(n)?;
                push(&mut stack, Obj::Bytes(r.take(n)?.to_vec()))
            }
            b'C' => {
                let n = r.byte()? as u64;
                let n = r.len(n)?;
                push(&mut stack, Obj::Bytes(r.take(n)?.to_vec()))
            }
            0x8e => {
                let n = r.u64()?;
                let n = r.len(n)?;
                push(&mut stack, Obj::Bytes(r.take(n)?.to_vec()))
            }
            b'}' => push(&mut stack, Obj::Dict(Vec::new())),
            b']' => push(&mut stack, Obj::List(Vec::new())),
            b')' => push(&mut stack, Obj::Tuple(Vec::new())),
            b'(' => stack.push(Slot::Mark),
            b'0' => {
                pop(&mut stack)?;
            }
            b'1' => {
                pop_mark(&mut stack)?;
            }
            b'2' => {
                let top = stack.last().cloned().ok_or_else(|| PickleError("unpickling stack underflow".into()))?;
                if matches!(top, Slot::Val(Obj::List(_) | Obj::Dict(_), _)) {
                    return err("a shared list or dict is not supported");
                }
                stack.push(top);
            }
            0x85..=0x87 => {
                let n = (op - 0x84) as usize;
                if stack.len() < n {
                    return err("unpickling stack underflow");
                }
                let items: Vec<(Obj, u16)> = (0..n).map(|_| pop(&mut stack)).collect::<Result<Vec<_>, _>>()?.into_iter().rev().collect();
                let d = depth_of(&items)?;
                stack.push(Slot::Val(Obj::Tuple(items.into_iter().map(|(o, _)| o).collect()), d));
            }
            b't' | b'l' => {
                let items = pop_mark(&mut stack)?;
                let d = depth_of(&items)?;
                let v: Vec<Obj> = items.into_iter().map(|(o, _)| o).collect();
                stack.push(Slot::Val(if op == b't' { Obj::Tuple(v) } else { Obj::List(v) }, d));
            }
            b'd' => {
                let items = pop_mark(&mut stack)?;
                if items.len() % 2 != 0 {
                    return err("odd number of items for a dict");
                }
                let d = depth_of(&items)?;
                let mut pairs: Vec<(Obj, Obj)> = Vec::new();
                let mut it = items.into_iter().map(|(o, _)| o);
                while let (Some(k), Some(v)) = (it.next(), it.next()) {
                    set_item(&mut pairs, k, v)?;
                }
                stack.push(Slot::Val(Obj::Dict(pairs), d));
            }
            b'a' | b'e' => {
                let items = if op == b'a' { vec![pop(&mut stack)?] } else { pop_mark(&mut stack)? };
                match stack.last_mut() {
                    Some(Slot::Val(Obj::List(l), d)) => {
                        *d = (*d).max(depth_of(&items)?);
                        l.extend(items.into_iter().map(|(o, _)| o));
                    }
                    _ => return err("APPEND target is not a list"),
                }
            }
            b's' | b'u' => {
                let items = if op == b's' {
                    let v = pop(&mut stack)?;
                    let k = pop(&mut stack)?;
                    vec![k, v]
                } else {
                    pop_mark(&mut stack)?
                };
                if items.len() % 2 != 0 {
                    return err("odd number of items for SETITEMS");
                }
                match stack.last_mut() {
                    Some(Slot::Val(Obj::Dict(pairs), d)) => {
                        *d = (*d).max(depth_of(&items)?);
                        let mut it = items.into_iter().map(|(o, _)| o);
                        while let (Some(k), Some(v)) = (it.next(), it.next()) {
                            set_item(pairs, k, v)?;
                        }
                    }
                    _ => return err("SETITEM target is not a dict"),
                }
            }
            b'q' => {
                let k = r.byte()? as u64;
                memoize(&mut memo, k, stack.last())?;
            }
            b'r' => {
                let k = r.u32()?;
                memoize(&mut memo, k, stack.last())?;
            }
            b'p' => {
                let l = r.line()?;
                let k: u64 = latin1(l).trim().parse().or_else(|_| err("invalid memo key"))?;
                memoize(&mut memo, k, stack.last())?;
            }
            0x94 => {
                memoize(&mut memo, next_memo, stack.last())?;
                next_memo += 1;
            }
            b'h' => {
                let k = r.byte()? as u64;
                stack.push(fetch(&memo, k)?);
            }
            b'j' => {
                let k = r.u32()?;
                stack.push(fetch(&memo, k)?);
            }
            b'g' => {
                let l = r.line()?;
                let k: u64 = latin1(l).trim().parse().or_else(|_| err("invalid memo key"))?;
                stack.push(fetch(&memo, k)?);
            }
            other => return err(format!("blocked opcode 0x{other:02x}")),
        }
    }
}

fn set_item(pairs: &mut Vec<(Obj, Obj)>, k: Obj, v: Obj) -> Result<(), PickleError> {
    if !hashable(&k) {
        return err("unhashable dict key");
    }
    match pairs.iter_mut().find(|(ek, _)| *ek == k) {
        Some(slot) => slot.1 = v,
        None => pairs.push((k, v)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // pickle.dumps({"a/b.png": [(10, 4, "")], "c": [(1, 2, b"")]}, protocol=2) is not available without Python, so the opcodes are written out.
    #[test]
    fn a_protocol_2_index_is_read() {
        let mut d =
            vec![0x80, 2, b'}', b'q', 0, b'(', b'U', 3, b'a', b'.', b'b', b'q', 1, b']', b'q', 2, b'K', 10, b'K', 4, b'U', 0, 0x87, b'q', 3, b'a', b'u', b'.'];
        let v = load(&d).unwrap();
        let Obj::Dict(p) = v else { panic!("not a dict") };
        assert_eq!(p[0].0, Obj::Str("a.b".into()));
        assert_eq!(p[0].1, Obj::List(vec![Obj::Tuple(vec![Obj::Int(10), Obj::Int(4), Obj::Str(String::new())])]));
        d.truncate(10);
        assert!(load(&d).is_err());
    }

    #[test]
    fn every_opcode_that_could_call_something_is_refused() {
        for op in [b'c', b'R', b'b', b'i', b'o', 0x93, 0x81, 0x82, 0x83, 0x84, 0x92] {
            let e = load(&[0x80, 2, op]).unwrap_err();
            assert!(e.0.contains("blocked"), "{op:#x}: {e:?}");
        }
    }

    #[test]
    fn a_huge_length_is_refused_before_allocating() {
        assert!(load(&[b'B', 0xff, 0xff, 0xff, 0x7f, 1, 2]).is_err());
        assert!(load(&[0x8e, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]).is_err());
    }

    #[test]
    fn deep_nesting_is_refused_without_overflowing_the_stack() {
        let mut d = vec![0x80, 2, b'N'];
        d.extend(std::iter::repeat_n(0x85, 1_000_000));
        d.push(b'.');
        assert!(load(&d).is_err());
    }

    #[test]
    fn a_list_cannot_be_a_dict_key() {
        let d = [0x80, 2, b'}', b'(', b']', b'N', b'u', b'.'];
        assert!(load(&d).is_err());
    }
}
