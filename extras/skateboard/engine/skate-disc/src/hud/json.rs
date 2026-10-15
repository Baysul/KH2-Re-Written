//! Insertion-ordered JSON values, written exactly as Python's
//! `json.dumps(value, separators=(',', ':'))` writes them: dict order kept,
//! `ensure_ascii` escapes and `repr(float)` numbers.
use std::fmt::Write;

#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Obj),
}

/// A Python dict: assigning an existing key keeps its first position.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Obj(pub Vec<(String, J)>);

impl Obj {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<J>) {
        let key = key.into();
        let value = value.into();
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.0.push((key, value)),
        }
    }
    /// `dict.setdefault`: only the first assignment counts.
    pub fn set_default(&mut self, key: impl Into<String>, value: impl Into<J>) {
        let key = key.into();
        if self.get(&key).is_none() {
            self.0.push((key, value.into()));
        }
    }
    pub fn get(&self, key: &str) -> Option<&J> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl J {
    pub fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(o) => o.get(key),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            J::Int(v) => Some(*v),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            J::Int(v) => Some(*v as f64),
            J::Float(v) => Some(*v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[J]> {
        match self {
            J::Arr(a) => Some(a),
            _ => None,
        }
    }
}

macro_rules! from_int {
    ($($t:ty),*) => {$(impl From<$t> for J {
        fn from(v: $t) -> Self { J::Int(v as i64) }
    })*};
}
from_int!(u8, u16, u32, i32, i64, usize);

impl From<f64> for J {
    fn from(v: f64) -> Self {
        J::Float(v)
    }
}
impl From<bool> for J {
    fn from(v: bool) -> Self {
        J::Bool(v)
    }
}
impl From<String> for J {
    fn from(v: String) -> Self {
        J::Str(v)
    }
}
impl From<&str> for J {
    fn from(v: &str) -> Self {
        J::Str(v.into())
    }
}
impl From<Obj> for J {
    fn from(v: Obj) -> Self {
        J::Obj(v)
    }
}
impl<T: Into<J>> From<Vec<T>> for J {
    fn from(v: Vec<T>) -> Self {
        J::Arr(v.into_iter().map(Into::into).collect())
    }
}
impl<T: Into<J>> From<Option<T>> for J {
    fn from(v: Option<T>) -> Self {
        v.map_or(J::Null, Into::into)
    }
}

/// Python's `repr(float)` (and json's float form, with NaN/Infinity allowed).
pub fn py_float(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0" } else { "0.0" }.into();
    }
    // Rust's shortest round-trip digits, in d.ddde<exp> form.
    let sci = format!("{:e}", x.abs());
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((&sci, "0"));
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let decpt = exponent + 1;
    let mut out = String::new();
    if x < 0.0 {
        out.push('-');
    }
    if !(-4 < decpt && decpt <= 16) {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let e = decpt - 1;
        let _ = write!(out, "e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs());
    } else if decpt <= 0 {
        out.push_str("0.");
        out.push_str(&"0".repeat((-decpt) as usize));
        out.push_str(&digits);
    } else if (decpt as usize) < digits.len() {
        out.push_str(&digits[..decpt as usize]);
        out.push('.');
        out.push_str(&digits[decpt as usize..]);
    } else {
        out.push_str(&digits);
        out.push_str(&"0".repeat(decpt as usize - digits.len()));
        out.push_str(".0");
    }
    out
}

fn py_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ' '..='~' => out.push(c),
            _ => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
    }
    out.push('"');
}

fn write_value(out: &mut String, value: &J) {
    match value {
        J::Null => out.push_str("null"),
        J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        J::Int(v) => {
            let _ = write!(out, "{v}");
        }
        J::Float(v) => out.push_str(&py_float(*v)),
        J::Str(s) => py_string(out, s),
        J::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        J::Obj(o) => {
            out.push('{');
            for (i, (k, v)) in o.0.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                py_string(out, k);
                out.push(':');
                write_value(out, v);
            }
            out.push('}');
        }
    }
}

/// `json.dumps(value, separators=(',', ':'))`.
pub fn dumps(value: &J) -> String {
    let mut out = String::new();
    write_value(&mut out, value);
    out
}

/// Python `str.isspace` (the characters `str.split()`/`str.strip()` use).
pub fn py_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n'
            | '\u{b}'
            | '\u{c}'
            | '\r'
            | '\u{1c}'..='\u{1f}'
            | ' '
            | '\u{85}'
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
    )
}

/// `str.strip()`.
pub fn py_strip(s: &str) -> &str {
    s.trim_matches(py_space)
}

/// `str.split()`.
pub fn py_split(s: &str) -> Vec<&str> {
    s.split(py_space).filter(|p| !p.is_empty()).collect()
}

/// `int(text)` for the decimal text these files hold.
pub fn py_int(text: &str) -> Option<i64> {
    let t = py_strip(text);
    let (negative, body) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    // Digits, optionally separated by single underscores.
    if body.is_empty()
        || body.starts_with('_')
        || body.ends_with('_')
        || body.contains("__")
        || !body.chars().all(|c| c.is_ascii_digit() || c == '_')
    {
        return None;
    }
    let value: i64 = body.replace('_', "").parse().ok()?;
    Some(if negative { -value } else { value })
}
