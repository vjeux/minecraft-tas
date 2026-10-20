//! Named binary tags (NBT): the tag tree, Java's big-endian binary form (write and read), and
//! SNBT, the text form the designs use (`nbt:` lines for block entities, `entity:` lines).
//! Compounds keep their insertion order, so a written file lists its keys in the order the
//! writer chose (Litematica's own order).

use anyhow::{Result, anyhow, bail};

#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    /// A list (all elements of one type; an empty list is written with element type End, as the
    /// game writes it).
    List(Vec<Tag>),
    Compound(Vec<(String, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Default for Tag {
    /// An empty compound.
    fn default() -> Tag {
        Tag::Compound(Vec::new())
    }
}

impl Tag {
    pub fn id(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::ByteArray(_) => 7,
            Tag::String(_) => 8,
            Tag::List(_) => 9,
            Tag::Compound(_) => 10,
            Tag::IntArray(_) => 11,
            Tag::LongArray(_) => 12,
        }
    }

    /// An empty compound.
    pub fn compound() -> Tag {
        Tag::Compound(Vec::new())
    }

    pub fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(v) => v.iter().find(|(k, _)| k == key).map(|(_, t)| t),
            _ => None,
        }
    }

    /// Set `key` (replacing its value in place, else appending).
    pub fn put(&mut self, key: &str, value: Tag) {
        if let Tag::Compound(v) = self {
            match v.iter_mut().find(|(k, _)| k == key) {
                Some((_, t)) => *t = value,
                None => v.push((key.to_string(), value)),
            }
        }
    }

    /// `put`, builder style.
    pub fn with(mut self, key: &str, value: Tag) -> Tag {
        self.put(key, value);
        self
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Tag::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Tag::Byte(v) => Some(v as i64),
            Tag::Short(v) => Some(v as i64),
            Tag::Int(v) => Some(v as i64),
            Tag::Long(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Tag::Float(v) => Some(v as f64),
            Tag::Double(v) => Some(v),
            _ => self.as_i64().map(|v| v as f64),
        }
    }

    pub fn as_list(&self) -> &[Tag] {
        match self {
            Tag::List(v) => v,
            _ => &[],
        }
    }

    pub fn entries(&self) -> &[(String, Tag)] {
        match self {
            Tag::Compound(v) => v,
            _ => &[],
        }
    }

    /// The SNBT text of the tag (keys quoted when needed, numbers with their suffixes).
    pub fn to_snbt(&self) -> String {
        let mut s = String::new();
        write_snbt(self, &mut s);
        s
    }
}

// ---- binary ------------------------------------------------------------------------------------

/// Java's modified UTF-8 (`DataOutput.writeUTF`): NUL as two bytes, characters beyond the
/// basic plane as two 3-byte surrogates; at most 65535 bytes.
fn put_string(out: &mut Vec<u8>, s: &str) -> Result<()> {
    let mut b: Vec<u8> = Vec::with_capacity(s.len());
    for c in s.chars() {
        let mut units = [0u16; 2];
        for &u in c.encode_utf16(&mut units).iter() {
            match u {
                0x0001..=0x007f => b.push(u as u8),
                0x0000 | 0x0080..=0x07ff => {
                    b.push(0xc0 | (u >> 6) as u8);
                    b.push(0x80 | (u & 0x3f) as u8);
                }
                _ => {
                    b.push(0xe0 | (u >> 12) as u8);
                    b.push(0x80 | ((u >> 6) & 0x3f) as u8);
                    b.push(0x80 | (u & 0x3f) as u8);
                }
            }
        }
    }
    if b.len() > u16::MAX as usize {
        bail!("NBT string of {} bytes (the format allows 65535)", b.len());
    }
    out.extend_from_slice(&(b.len() as u16).to_be_bytes());
    out.extend_from_slice(&b);
    Ok(())
}

fn put_payload(out: &mut Vec<u8>, t: &Tag) -> Result<()> {
    match t {
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Float(v) => out.extend_from_slice(&v.to_bits().to_be_bytes()),
        Tag::Double(v) => out.extend_from_slice(&v.to_bits().to_be_bytes()),
        Tag::ByteArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            out.extend(v.iter().map(|b| *b as u8));
        }
        Tag::String(s) => put_string(out, s)?,
        Tag::List(v) => {
            let elem = v.first().map(Tag::id).unwrap_or(0);
            if let Some(bad) = v.iter().find(|x| x.id() != elem) {
                bail!("NBT list mixes tag types {elem} and {}", bad.id());
            }
            out.push(elem);
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                put_payload(out, x)?;
            }
        }
        Tag::Compound(v) => {
            for (k, x) in v {
                out.push(x.id());
                put_string(out, k)?;
                put_payload(out, x)?;
            }
            out.push(0);
        }
        Tag::IntArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
        Tag::LongArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
    }
    Ok(())
}

/// The on-disk form of a root tag: its type, its name, its payload (uncompressed).
pub fn write_named(name: &str, root: &Tag) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.push(root.id());
    put_string(&mut out, name)?;
    put_payload(&mut out, root)?;
    Ok(out)
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.i.checked_add(n).filter(|e| *e <= self.b.len()).ok_or_else(|| anyhow!("NBT truncated at byte {}", self.i))?;
        let s = &self.b[self.i..end];
        self.i = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into()?))
    }
    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into()?))
    }
    fn len(&mut self) -> Result<usize> {
        let n = self.i32()?;
        if n < 0 {
            bail!("negative NBT length {n}");
        }
        Ok(n as usize)
    }
    /// Modified UTF-8 back to a string (surrogate pairs rejoined).
    fn string(&mut self) -> Result<String> {
        let n = u16::from_be_bytes(self.take(2)?.try_into()?) as usize;
        let b = self.take(n)?;
        let mut units: Vec<u16> = Vec::with_capacity(n);
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            if c & 0x80 == 0 {
                units.push(c as u16);
                i += 1;
            } else if c & 0xe0 == 0xc0 && i + 1 < b.len() {
                units.push(((c as u16 & 0x1f) << 6) | (b[i + 1] as u16 & 0x3f));
                i += 2;
            } else if c & 0xf0 == 0xe0 && i + 2 < b.len() {
                units.push(((c as u16 & 0x0f) << 12) | ((b[i + 1] as u16 & 0x3f) << 6) | (b[i + 2] as u16 & 0x3f));
                i += 3;
            } else {
                bail!("bad modified UTF-8 in an NBT string");
            }
        }
        Ok(String::from_utf16_lossy(&units))
    }
    fn payload(&mut self, kind: u8, depth: u32) -> Result<Tag> {
        if depth > 512 {
            bail!("NBT nested too deep");
        }
        Ok(match kind {
            1 => Tag::Byte(self.u8()? as i8),
            2 => Tag::Short(i16::from_be_bytes(self.take(2)?.try_into()?)),
            3 => Tag::Int(self.i32()?),
            4 => Tag::Long(self.i64()?),
            5 => Tag::Float(f32::from_bits(self.i32()? as u32)),
            6 => Tag::Double(f64::from_bits(self.i64()? as u64)),
            7 => {
                let n = self.len()?;
                Tag::ByteArray(self.take(n)?.iter().map(|&b| b as i8).collect())
            }
            8 => Tag::String(self.string()?),
            9 => {
                let elem = self.u8()?;
                let n = self.len()?;
                if elem == 0 && n > 0 {
                    bail!("NBT list of {n} End tags");
                }
                let mut v = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    v.push(self.payload(elem, depth + 1)?);
                }
                Tag::List(v)
            }
            10 => {
                let mut v = Vec::new();
                loop {
                    let k = self.u8()?;
                    if k == 0 {
                        break;
                    }
                    let name = self.string()?;
                    v.push((name, self.payload(k, depth + 1)?));
                }
                Tag::Compound(v)
            }
            11 => {
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    v.push(self.i32()?);
                }
                Tag::IntArray(v)
            }
            12 => {
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    v.push(self.i64()?);
                }
                Tag::LongArray(v)
            }
            k => bail!("unknown NBT tag type {k} at byte {}", self.i),
        })
    }
}

/// Read a named root tag (uncompressed): (name, tag).
pub fn read_named(bytes: &[u8]) -> Result<(String, Tag)> {
    let mut r = Reader { b: bytes, i: 0 };
    let kind = r.u8()?;
    let name = r.string()?;
    let tag = r.payload(kind, 0)?;
    if r.i != bytes.len() {
        bail!("{} bytes after the NBT root", bytes.len() - r.i);
    }
    Ok((name, tag))
}

// ---- SNBT ----------------------------------------------------------------------------------------

/// Parse SNBT (`{Items:[{Slot:0b,id:"minecraft:stone",count:3}]}`, `{Rotation:[90f,0f]}`): compounds,
/// lists, typed arrays (`[B;1b]`, `[I;1]`, `[L;1l]`), quoted and bare strings, numbers with the
/// suffixes b s l f d (bare: int, or double with a point or exponent), true and false (bytes).
pub fn parse_snbt(s: &str) -> Result<Tag> {
    let mut p = Snbt { s: s.as_bytes(), i: 0, text: s };
    let t = p.value(0)?;
    p.ws();
    if p.i != p.s.len() {
        bail!("SNBT: unexpected {:?} at {} in {s:?}", &s[p.i..], p.i);
    }
    Ok(t)
}

struct Snbt<'a> {
    s: &'a [u8],
    i: usize,
    text: &'a str,
}

impl Snbt<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] as char).is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn err(&self, what: &str) -> anyhow::Error {
        anyhow!("SNBT: {what} at {} in {:?}", self.i, self.text)
    }
    fn expect(&mut self, c: u8) -> Result<()> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err(&format!("expected {:?}", c as char)))
        }
    }
    /// A bare word: [0-9A-Za-z_-.+] (vanilla's unquoted strings; a colon needs quotes).
    fn bare(&mut self) -> &str {
        let start = self.i;
        while self.i < self.s.len() {
            let c = self.s[self.i] as char;
            if c.is_ascii_alphanumeric() || "_-.+".contains(c) {
                self.i += 1;
            } else {
                break;
            }
        }
        &self.text[start..self.i]
    }
    fn quoted(&mut self) -> Result<String> {
        let q = self.s[self.i];
        self.i += 1;
        let mut out = String::new();
        let start = self.i;
        let mut chunk_start = start;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c == b'\\' {
                out.push_str(&self.text[chunk_start..self.i]);
                let e = *self.s.get(self.i + 1).ok_or_else(|| self.err("dangling escape"))?;
                out.push(match e {
                    b'n' => '\n',
                    b't' => '\t',
                    b'r' => '\r',
                    other => other as char,
                });
                self.i += 2;
                chunk_start = self.i;
            } else if c == q {
                out.push_str(&self.text[chunk_start..self.i]);
                self.i += 1;
                return Ok(out);
            } else {
                self.i += 1;
            }
        }
        Err(self.err("unterminated string"))
    }
    fn key(&mut self) -> Result<String> {
        match self.peek() {
            Some(b'"') | Some(b'\'') => self.quoted(),
            _ => {
                let k = self.bare().to_string();
                if k.is_empty() { Err(self.err("expected a key")) } else { Ok(k) }
            }
        }
    }
    fn value(&mut self, depth: u32) -> Result<Tag> {
        if depth > 256 {
            return Err(self.err("nested too deep"));
        }
        match self.peek() {
            Some(b'{') => {
                self.i += 1;
                let mut v: Vec<(String, Tag)> = Vec::new();
                if self.peek() == Some(b'}') {
                    self.i += 1;
                    return Ok(Tag::Compound(v));
                }
                loop {
                    let k = self.key()?;
                    self.expect(b':')?;
                    let x = self.value(depth + 1)?;
                    match v.iter_mut().find(|(kk, _)| *kk == k) {
                        Some((_, t)) => *t = x,
                        None => v.push((k, x)),
                    }
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Tag::Compound(v));
                        }
                        _ => return Err(self.err("expected ',' or '}'")),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                // typed arrays: [B; ...], [I; ...], [L; ...]
                let save = self.i;
                self.ws();
                if self.i + 1 < self.s.len() && matches!(self.s[self.i], b'B' | b'I' | b'L') && self.s[self.i + 1] == b';' {
                    let kind = self.s[self.i];
                    self.i += 2;
                    let mut nums: Vec<i64> = Vec::new();
                    if self.peek() != Some(b']') {
                        loop {
                            let t = self.value(depth + 1)?;
                            nums.push(t.as_i64().ok_or_else(|| self.err("non-integer in a typed array"))?);
                            match self.peek() {
                                Some(b',') => self.i += 1,
                                Some(b']') => break,
                                _ => return Err(self.err("expected ',' or ']'")),
                            }
                        }
                    }
                    self.i += 1;
                    return Ok(match kind {
                        b'B' => Tag::ByteArray(nums.iter().map(|v| *v as i8).collect()),
                        b'I' => Tag::IntArray(nums.iter().map(|v| *v as i32).collect()),
                        _ => Tag::LongArray(nums),
                    });
                }
                self.i = save;
                let mut v: Vec<Tag> = Vec::new();
                if self.peek() == Some(b']') {
                    self.i += 1;
                    return Ok(Tag::List(v));
                }
                loop {
                    v.push(self.value(depth + 1)?);
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            break;
                        }
                        _ => return Err(self.err("expected ',' or ']'")),
                    }
                }
                if let Some(first) = v.first() {
                    let id = first.id();
                    if v.iter().any(|x| x.id() != id) {
                        return Err(self.err("a list mixes tag types"));
                    }
                }
                Ok(Tag::List(v))
            }
            Some(b'"') | Some(b'\'') => Ok(Tag::String(self.quoted()?)),
            Some(_) => {
                let word = self.bare().to_string();
                if word.is_empty() {
                    return Err(self.err("expected a value"));
                }
                Ok(scalar(&word))
            }
            None => Err(self.err("unexpected end")),
        }
    }
}

/// A bare SNBT word: a number with its type, a boolean, else a string.
fn scalar(w: &str) -> Tag {
    match w {
        "true" => return Tag::Byte(1),
        "false" => return Tag::Byte(0),
        _ => {}
    }
    let (body, suffix) = match w.char_indices().last() {
        Some((i, c)) if "bBsSlLfFdD".contains(c) && i > 0 => (&w[..i], Some(c.to_ascii_lowercase())),
        _ => (w, None),
    };
    let int = |s: &str| s.parse::<i64>().ok();
    let float = |s: &str| s.parse::<f64>().ok().filter(|_| s.chars().all(|c| c.is_ascii_digit() || "+-.eE".contains(c)));
    match suffix {
        Some('b') => int(body).map(|v| Tag::Byte(v as i8)),
        Some('s') => int(body).map(|v| Tag::Short(v as i16)),
        Some('l') => int(body).map(Tag::Long),
        Some('f') => float(body).map(|v| Tag::Float(v as f32)),
        Some('d') => float(body).map(Tag::Double),
        _ => None,
    }
    .or_else(|| {
        if suffix.is_some() {
            return None;
        }
        if let Some(v) = int(w) {
            return Some(if let Ok(i) = i32::try_from(v) { Tag::Int(i) } else { Tag::Long(v) });
        }
        if w.contains(['.', 'e', 'E']) {
            return float(w).map(Tag::Double);
        }
        None
    })
    .unwrap_or_else(|| Tag::String(w.to_string()))
}

fn write_snbt(t: &Tag, s: &mut String) {
    let key = |k: &str| -> String {
        if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || "_-.+".contains(c)) { k.to_string() } else { quote(k) }
    };
    match t {
        Tag::Byte(v) => s.push_str(&format!("{v}b")),
        Tag::Short(v) => s.push_str(&format!("{v}s")),
        Tag::Int(v) => s.push_str(&v.to_string()),
        Tag::Long(v) => s.push_str(&format!("{v}L")),
        Tag::Float(v) => s.push_str(&format!("{v:?}f")),
        Tag::Double(v) => s.push_str(&format!("{v:?}d")),
        Tag::ByteArray(v) => s.push_str(&format!("[B;{}]", v.iter().map(|x| format!("{x}B")).collect::<Vec<_>>().join(","))),
        Tag::String(v) => s.push_str(&quote(v)),
        Tag::List(v) => {
            s.push('[');
            for (i, x) in v.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                write_snbt(x, s);
            }
            s.push(']');
        }
        Tag::Compound(v) => {
            s.push('{');
            for (i, (k, x)) in v.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&key(k));
                s.push(':');
                write_snbt(x, s);
            }
            s.push('}');
        }
        Tag::IntArray(v) => s.push_str(&format!("[I;{}]", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","))),
        Tag::LongArray(v) => s.push_str(&format!("[L;{}]", v.iter().map(|x| format!("{x}L")).collect::<Vec<_>>().join(","))),
    }
}

fn quote(v: &str) -> String {
    format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snbt_of_the_designs() {
        let t = parse_snbt("{Items:[{Slot:0b,id:\"minecraft:stone\",count:3}]}").unwrap();
        let item = &t.get("Items").unwrap().as_list()[0];
        assert_eq!(item.get("Slot"), Some(&Tag::Byte(0)));
        assert_eq!(item.get("id"), Some(&Tag::String("minecraft:stone".into())));
        assert_eq!(item.get("count"), Some(&Tag::Int(3)));
        let r = parse_snbt("{Rotation:[90f,0f]}").unwrap();
        assert_eq!(r.get("Rotation"), Some(&Tag::List(vec![Tag::Float(90.0), Tag::Float(0.0)])));
        let m = parse_snbt("{Motion:[0.4d,0.0d,0.0d], Invulnerable:1b, Name:'a \\'b\\'', ok:true, n:-3, big:5000000000, x:1.5, l:7L, arr:[I;1,2], b:[B;1b]}").unwrap();
        assert_eq!(m.get("Motion").unwrap().as_list().len(), 3);
        assert_eq!(m.get("Invulnerable"), Some(&Tag::Byte(1)));
        assert_eq!(m.get("Name"), Some(&Tag::String("a 'b'".into())));
        assert_eq!(m.get("ok"), Some(&Tag::Byte(1)));
        assert_eq!(m.get("n"), Some(&Tag::Int(-3)));
        assert_eq!(m.get("big"), Some(&Tag::Long(5_000_000_000)));
        assert_eq!(m.get("x"), Some(&Tag::Double(1.5)));
        assert_eq!(m.get("l"), Some(&Tag::Long(7)));
        assert_eq!(m.get("arr"), Some(&Tag::IntArray(vec![1, 2])));
        assert_eq!(m.get("b"), Some(&Tag::ByteArray(vec![1])));
        assert!(parse_snbt("{a:[1,2b]}").is_err());
        assert!(parse_snbt("{a:1").is_err());
        assert_eq!(parse_snbt("{}").unwrap(), Tag::compound());
    }

    #[test]
    fn binary_round_trip() {
        let t = parse_snbt("{a:1b,b:2s,c:3,d:4L,e:5.5f,f:6.25d,g:[B;1b,2b],h:\"h\\u00e9 · ✓ 🧱\",i:[{x:1}],j:[],k:[I;1],l:[L;2L],m:{n:\"\"}}").unwrap();
        let mut t = t;
        t.put("h", Tag::String("hé · ✓ 🧱 \u{0}".into()));
        let bytes = write_named("root", &t).unwrap();
        let (name, back) = read_named(&bytes).unwrap();
        assert_eq!(name, "root");
        assert_eq!(back, t);
        // the empty list "j" is written with element type End (0), like the game
        let j = bytes.windows(4).position(|w| w == [9, 0, 1, b'j']).unwrap();
        assert_eq!(&bytes[j + 4..j + 9], &[0, 0, 0, 0, 0]);
        // the repo's independent reader (mctas::vanilla::nbt) agrees
        let other = mctas::vanilla::nbt::parse_named(&bytes).unwrap();
        assert_eq!(other.get("c").and_then(|x| x.as_i64()), Some(3));
        assert_eq!(other.get("h").and_then(|x| x.as_str()).map(|s| s.starts_with("h")), Some(true));
        // snbt text back
        assert_eq!(parse_snbt(&back.to_snbt()).unwrap(), back);
    }
}
