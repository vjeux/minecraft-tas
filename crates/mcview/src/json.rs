//! A small JSON reader (no dependencies, so it compiles to wasm): enough for the real-game traces
//! (`designs/clocks/traces/*.trace`, JSON Lines).

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn parse(s: &str) -> Result<Json, String> {
        let b = s.as_bytes();
        let mut i = 0;
        let v = value(b, &mut i)?;
        skip_ws(b, &mut i);
        if i != b.len() {
            return Err(format!("trailing characters at {i}"));
        }
        Ok(v)
    }

    pub fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(kv) => kv.iter().find(|(kk, _)| kk == k).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        self.as_f64().map(|n| n as i64)
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }
    /// `[x, y, z]` of integers.
    pub fn as_pos(&self) -> Option<[i32; 3]> {
        let a = self.as_arr()?;
        if a.len() < 3 {
            return None;
        }
        Some([a[0].as_i64()? as i32, a[1].as_i64()? as i32, a[2].as_i64()? as i32])
    }
}

fn skip_ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && b[*i].is_ascii_whitespace() {
        *i += 1;
    }
}

fn value(b: &[u8], i: &mut usize) -> Result<Json, String> {
    skip_ws(b, i);
    let c = *b.get(*i).ok_or("unexpected end")?;
    match c {
        b'{' => {
            *i += 1;
            let mut kv = Vec::new();
            skip_ws(b, i);
            if b.get(*i) == Some(&b'}') {
                *i += 1;
                return Ok(Json::Obj(kv));
            }
            loop {
                skip_ws(b, i);
                let k = match value(b, i)? {
                    Json::Str(s) => s,
                    _ => return Err(format!("object key expected at {i}")),
                };
                skip_ws(b, i);
                if b.get(*i) != Some(&b':') {
                    return Err(format!("':' expected at {i}"));
                }
                *i += 1;
                kv.push((k, value(b, i)?));
                skip_ws(b, i);
                match b.get(*i) {
                    Some(b',') => *i += 1,
                    Some(b'}') => {
                        *i += 1;
                        return Ok(Json::Obj(kv));
                    }
                    _ => return Err(format!("',' or '}}' expected at {i}")),
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut a = Vec::new();
            skip_ws(b, i);
            if b.get(*i) == Some(&b']') {
                *i += 1;
                return Ok(Json::Arr(a));
            }
            loop {
                a.push(value(b, i)?);
                skip_ws(b, i);
                match b.get(*i) {
                    Some(b',') => *i += 1,
                    Some(b']') => {
                        *i += 1;
                        return Ok(Json::Arr(a));
                    }
                    _ => return Err(format!("',' or ']' expected at {i}")),
                }
            }
        }
        b'"' => {
            *i += 1;
            let mut s = String::new();
            loop {
                let c = *b.get(*i).ok_or("unterminated string")?;
                *i += 1;
                match c {
                    b'"' => return Ok(Json::Str(s)),
                    b'\\' => {
                        let e = *b.get(*i).ok_or("bad escape")?;
                        *i += 1;
                        match e {
                            b'n' => s.push('\n'),
                            b't' => s.push('\t'),
                            b'r' => s.push('\r'),
                            b'b' => s.push('\u{8}'),
                            b'f' => s.push('\u{c}'),
                            b'u' => {
                                let h = std::str::from_utf8(b.get(*i..*i + 4).ok_or("bad \\u")?).map_err(|e| e.to_string())?;
                                let cp = u32::from_str_radix(h, 16).map_err(|e| e.to_string())?;
                                *i += 4;
                                s.push(char::from_u32(cp).unwrap_or('?'));
                            }
                            other => s.push(other as char),
                        }
                    }
                    _ => {
                        // copy a UTF-8 run as is
                        let start = *i - 1;
                        let mut end = *i;
                        while end < b.len() && b[end] != b'"' && b[end] != b'\\' {
                            end += 1;
                        }
                        s.push_str(std::str::from_utf8(&b[start..end]).map_err(|e| e.to_string())?);
                        *i = end;
                    }
                }
            }
        }
        b't' if b[*i..].starts_with(b"true") => {
            *i += 4;
            Ok(Json::Bool(true))
        }
        b'f' if b[*i..].starts_with(b"false") => {
            *i += 5;
            Ok(Json::Bool(false))
        }
        b'n' if b[*i..].starts_with(b"null") => {
            *i += 4;
            Ok(Json::Null)
        }
        _ => {
            let start = *i;
            while *i < b.len() && (b[*i].is_ascii_digit() || matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E')) {
                *i += 1;
            }
            let t = std::str::from_utf8(&b[start..*i]).map_err(|e| e.to_string())?;
            t.parse::<f64>().map(Json::Num).map_err(|_| format!("bad value {t:?} at {start}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_trace_line() {
        let v = Json::parse(r#"{"t":3,"blocks":[[1,-2,3,"minecraft:water[level=1]"]],"entities":[{"id":0,"pos":[0.5,1e-3,-2.25],"type":"minecraft:oak_boat","yaw":-90.5}],"ok":true,"none":null,"s":"a\"b\u00e9"}"#).unwrap();
        assert_eq!(v.get("t").and_then(Json::as_i64), Some(3));
        let b = &v.get("blocks").unwrap().as_arr().unwrap()[0];
        assert_eq!(b.as_pos(), Some([1, -2, 3]));
        assert_eq!(b.as_arr().unwrap()[3].as_str(), Some("minecraft:water[level=1]"));
        let e = &v.get("entities").unwrap().as_arr().unwrap()[0];
        assert_eq!(e.get("yaw").and_then(Json::as_f64), Some(-90.5));
        assert_eq!(v.get("s").and_then(Json::as_str), Some("a\"bé"));
    }
}
