//! Insertion-ordered JSON value plus a writer that mimics Python's `json.dumps`
//! (`indent=2`, `ensure_ascii`, and the separator styles), so reports stay
//! byte-comparable with the Python originals.

#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn obj() -> J {
        J::Obj(Vec::new())
    }
    pub fn arr() -> J {
        J::Arr(Vec::new())
    }
    /// Append (or replace) a key, keeping first-insertion order like a Python dict.
    pub fn set(&mut self, key: &str, value: impl Into<J>) -> &mut Self {
        if let J::Obj(items) = self {
            let value = value.into();
            if let Some(slot) = items.iter_mut().find(|(k, _)| k == key) {
                slot.1 = value;
            } else {
                items.push((key.to_string(), value));
            }
        } else {
            panic!("J::set on non-object");
        }
        self
    }
    pub fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_i128(&self) -> Option<i128> {
        match self {
            J::Int(v) => Some(*v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn len(&self) -> usize {
        match self {
            J::Arr(a) => a.len(),
            J::Obj(o) => o.len(),
            _ => 0,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn push(&mut self, value: impl Into<J>) {
        if let J::Arr(items) = self {
            items.push(value.into());
        } else {
            panic!("J::push on non-array");
        }
    }
    /// Python `json.dumps(self, indent=2, ensure_ascii=...)` (no trailing newline).
    pub fn pretty(&self, ensure_ascii: bool) -> String {
        let mut out = String::new();
        write_value(self, &mut out, Some(2), 0, ensure_ascii, (",", ": "));
        out
    }
    /// Python `json.dumps(self, separators=(',', ':'))`.
    pub fn compact(&self, ensure_ascii: bool) -> String {
        let mut out = String::new();
        write_value(self, &mut out, None, 0, ensure_ascii, (",", ":"));
        out
    }
    /// Python default `json.dumps(self)` (`", "` and `": "` separators).
    pub fn line(&self, ensure_ascii: bool) -> String {
        let mut out = String::new();
        write_value(self, &mut out, None, 0, ensure_ascii, (", ", ": "));
        out
    }
}

/// Python `repr(float)` (shortest round-trip digits, exponent form outside `1e-4 <= |x| < 1e16`).
pub fn float_repr(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    // `{:e}` yields the shortest round-trip digits as `d.ddde<exp>`.
    let sci = format!("{:e}", value.abs());
    let (mantissa, exponent) = sci.split_once('e').expect("scientific format");
    let exponent: i32 = exponent.parse().expect("exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let sign = if value < 0.0 { "-" } else { "" };
    if (-4..16).contains(&exponent) {
        if exponent >= 0 {
            let int_len = exponent as usize + 1;
            if digits.len() <= int_len {
                format!("{sign}{digits}{}.0", "0".repeat(int_len - digits.len()))
            } else {
                format!("{sign}{}.{}", &digits[..int_len], &digits[int_len..])
            }
        } else {
            format!("{sign}0.{}{digits}", "0".repeat((-exponent - 1) as usize))
        }
    } else {
        let mant = if digits.len() > 1 { format!("{}.{}", &digits[..1], &digits[1..]) } else { digits };
        format!("{sign}{mant}e{}{:02}", if exponent < 0 { '-' } else { '+' }, exponent.abs())
    }
}

fn escape(text: &str, out: &mut String, ensure_ascii: bool) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if ensure_ascii && (c as u32) > 0x7e => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{:04x}", unit));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_value(
    value: &J,
    out: &mut String,
    indent: Option<usize>,
    level: usize,
    ensure_ascii: bool,
    seps: (&str, &str),
) {
    match value {
        J::Null => out.push_str("null"),
        J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        J::Int(i) => out.push_str(&i.to_string()),
        J::Float(f) => out.push_str(&float_repr(*f)),
        J::Str(s) => escape(s, out, ensure_ascii),
        J::Arr(items) if items.is_empty() => out.push_str("[]"),
        J::Obj(items) if items.is_empty() => out.push_str("{}"),
        J::Arr(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(seps.0);
                }
                newline(out, indent, level + 1);
                write_value(item, out, indent, level + 1, ensure_ascii, seps);
            }
            newline(out, indent, level);
            out.push(']');
        }
        J::Obj(items) => {
            out.push('{');
            for (index, (key, item)) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(seps.0);
                }
                newline(out, indent, level + 1);
                escape(key, out, ensure_ascii);
                out.push_str(seps.1);
                write_value(item, out, indent, level + 1, ensure_ascii, seps);
            }
            newline(out, indent, level);
            out.push('}');
        }
    }
}

fn newline(out: &mut String, indent: Option<usize>, level: usize) {
    if let Some(width) = indent {
        out.push('\n');
        for _ in 0..width * level {
            out.push(' ');
        }
    }
}

macro_rules! from_int {
    ($($t:ty),*) => {$(impl From<$t> for J { fn from(v: $t) -> J { J::Int(v as i128) } })*};
}
from_int!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, i128, u128);
impl From<bool> for J {
    fn from(v: bool) -> J {
        J::Bool(v)
    }
}
impl From<f64> for J {
    fn from(v: f64) -> J {
        J::Float(v)
    }
}
impl From<&str> for J {
    fn from(v: &str) -> J {
        J::Str(v.to_string())
    }
}
impl From<String> for J {
    fn from(v: String) -> J {
        J::Str(v)
    }
}
impl From<&String> for J {
    fn from(v: &String) -> J {
        J::Str(v.clone())
    }
}
impl<T: Into<J>> From<Vec<T>> for J {
    fn from(v: Vec<T>) -> J {
        J::Arr(v.into_iter().map(Into::into).collect())
    }
}
impl<T: Into<J>> From<Option<T>> for J {
    fn from(v: Option<T>) -> J {
        v.map_or(J::Null, Into::into)
    }
}

/// `jobj!{ "key" => value, ... }` builds an ordered object.
#[macro_export]
macro_rules! jobj {
    ($($k:expr => $v:expr),* $(,)?) => {{
        let mut o = $crate::json::J::obj();
        $( o.set($k, $v); )*
        o
    }};
}

/// Insertion-ordered counter; touching a key with `add(key, 0)` still inserts it
/// (Python `counts[k] += False` creates the key).
#[derive(Clone, Debug, Default)]
pub struct Counter {
    items: Vec<(String, i128)>,
}

impl Counter {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, key: impl AsRef<str>, n: impl Into<i128>) {
        let key = key.as_ref();
        let n = n.into();
        if let Some(slot) = self.items.iter_mut().find(|(k, _)| k == key) {
            slot.1 += n;
        } else {
            self.items.push((key.to_string(), n));
        }
    }
    /// Assignment (`counts[key] = n`), as opposed to [`Counter::add`].
    pub fn set(&mut self, key: impl AsRef<str>, n: i128) {
        let key = key.as_ref();
        if let Some(slot) = self.items.iter_mut().find(|(k, _)| k == key) {
            slot.1 = n;
        } else {
            self.items.push((key.to_string(), n));
        }
    }
    pub fn inc(&mut self, key: impl AsRef<str>) {
        self.add(key, 1i128);
    }
    pub fn get(&self, key: &str) -> i128 {
        self.items.iter().find(|(k, _)| k == key).map_or(0, |(_, v)| *v)
    }
    pub fn update(&mut self, other: &Counter) {
        for (k, v) in &other.items {
            self.add(k, *v);
        }
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn to_json(&self) -> J {
        J::Obj(self.items.iter().map(|(k, v)| (k.clone(), J::Int(*v))).collect())
    }
    pub fn items(&self) -> &[(String, i128)] {
        &self.items
    }
}
