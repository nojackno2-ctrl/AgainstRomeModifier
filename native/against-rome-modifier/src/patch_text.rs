//! Game text document reader and writer ported from src.Core/Core/Patches/PatchText.cs.
//! Supports Windows-1251 encoding round-trip, PFIL compression, CSV parsing, and length checking.

use crate::lzss::{compress_pfil, decompress_if_pfil, is_pfil};
use std::io;

// Mapping table for Windows-1251 byte (0x80..=0xFF) -> Unicode char
const CP1251_TABLE: [char; 128] = [
    '\u{0402}', '\u{0403}', '\u{201A}', '\u{0453}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{20AC}', '\u{2030}', '\u{0409}', '\u{2039}', '\u{040A}', '\u{040C}', '\u{040B}', '\u{040F}',
    '\u{0452}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{0098}', '\u{2122}', '\u{0459}', '\u{203A}', '\u{045A}', '\u{045C}', '\u{045B}', '\u{045F}',
    '\u{00A0}', '\u{040E}', '\u{045E}', '\u{0408}', '\u{00A4}', '\u{0490}', '\u{00A6}', '\u{00A7}',
    '\u{0401}', '\u{00A9}', '\u{0404}', '\u{00AB}', '\u{00AC}', '\u{00AD}', '\u{00AE}', '\u{0407}',
    '\u{00B0}', '\u{00B1}', '\u{0406}', '\u{0456}', '\u{0491}', '\u{00B5}', '\u{00B6}', '\u{00B7}',
    '\u{0451}', '\u{2116}', '\u{0454}', '\u{00BB}', '\u{0458}', '\u{0405}', '\u{0455}', '\u{0457}',
    '\u{0410}', '\u{0411}', '\u{0412}', '\u{0413}', '\u{0414}', '\u{0415}', '\u{0416}', '\u{0417}',
    '\u{0418}', '\u{0419}', '\u{041A}', '\u{041B}', '\u{041C}', '\u{041D}', '\u{041E}', '\u{041F}',
    '\u{0420}', '\u{0421}', '\u{0422}', '\u{0423}', '\u{0424}', '\u{0425}', '\u{0426}', '\u{0427}',
    '\u{0428}', '\u{0429}', '\u{042A}', '\u{042B}', '\u{042C}', '\u{042D}', '\u{042E}', '\u{042F}',
    '\u{0430}', '\u{0431}', '\u{0432}', '\u{0433}', '\u{0434}', '\u{0435}', '\u{0436}', '\u{0437}',
    '\u{0438}', '\u{0439}', '\u{043A}', '\u{043B}', '\u{043C}', '\u{043D}', '\u{043E}', '\u{043F}',
    '\u{0440}', '\u{0441}', '\u{0442}', '\u{0443}', '\u{0444}', '\u{0445}', '\u{0446}', '\u{0447}',
    '\u{0448}', '\u{0449}', '\u{044A}', '\u{044B}', '\u{044C}', '\u{044D}', '\u{044E}', '\u{044F}',
];

pub fn decode_cp1251(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len());
    for &b in bytes {
        if b < 0x80 {
            s.push(b as char);
        } else {
            s.push(CP1251_TABLE[(b - 0x80) as usize]);
        }
    }
    s
}

pub fn encode_cp1251(s: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(s.len());
    for ch in s.chars() {
        let code = ch as u32;
        if code < 0x80 {
            bytes.push(code as u8);
        } else if let Some(idx) = CP1251_TABLE.iter().position(|&c| c == ch) {
            bytes.push((0x80 + idx) as u8);
        } else {
            // Fallback for non-representable chars: ascii '?'
            bytes.push(b'?');
        }
    }
    bytes
}

#[derive(Debug, Clone)]
pub struct TextDocument {
    pub text: String,
    pub line_ending: &'static str,
    pub lines: Vec<String>,
    pub was_pfil: bool,
    pub raw_header_template: Option<Vec<u8>>,
}

impl TextDocument {
    pub fn read(raw_bytes: &[u8]) -> io::Result<Self> {
        let was_pfil = is_pfil(raw_bytes);
        let header_template = if was_pfil && raw_bytes.len() >= 64 {
            Some(raw_bytes[..64].to_vec())
        } else {
            None
        };

        let decompressed = decompress_if_pfil(raw_bytes)?;
        let text = decode_cp1251(&decompressed);

        let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };

        let lines: Vec<String> = text.split(line_ending).map(|s| s.to_string()).collect();

        Ok(Self {
            text,
            line_ending,
            lines,
            was_pfil,
            raw_header_template: header_template,
        })
    }

    pub fn write_with_lines(&self, lines: &[String]) -> io::Result<Vec<u8>> {
        let mut text = lines.join(self.line_ending);
        if self.text.ends_with(self.line_ending) && !text.ends_with(self.line_ending) {
            text.push_str(self.line_ending);
        }

        let encoded = encode_cp1251(&text);
        if self.was_pfil {
            compress_pfil(&encoded, self.raw_header_template.as_deref())
        } else {
            Ok(encoded)
        }
    }
}

pub fn parse_csv_line(line: &str) -> Vec<String> {
    line.split(',').map(|s| s.to_string()).collect()
}

pub fn to_csv_string<'a>(columns: impl IntoIterator<Item = &'a str>) -> String {
    let cols: Vec<&'a str> = columns.into_iter().collect();
    cols.join(",")
}

pub fn parse_double_col(columns: &[String], index: usize) -> f64 {
    if index < columns.len() {
        parse_double(&columns[index])
    } else {
        0.0
    }
}

pub fn parse_double(val: &str) -> f64 {
    val.trim().parse::<f64>().unwrap_or(0.0)
}

pub fn check_length(value: &str, target_length: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.len() <= target_length {
        return Some(trimmed.to_string());
    }

    if let Ok(number) = trimmed.parse::<f64>() {
        let one_decimal = format!("{:.1}", number);
        if one_decimal.len() <= target_length {
            return Some(one_decimal);
        }

        let integer = format!("{:.0}", number.round());
        if integer.len() <= target_length {
            return Some(integer);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cp1251_ascii_and_cyrillic_roundtrip() {
        let original = "Hello мир 123! [objres]\r\n";
        let encoded = encode_cp1251(original);
        let decoded = decode_cp1251(&encoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn check_length_fits_or_rounds() {
        assert_eq!(check_length("12.345", 10), Some("12.345".to_string()));
        assert_eq!(check_length("12.345", 4), Some("12.3".to_string()));
        assert_eq!(check_length("12.999", 2), Some("13".to_string()));
        assert_eq!(check_length("123456", 3), None);
    }

    #[test]
    fn text_document_preserves_pfil_state() {
        let raw = b"[section]\r\nkey=val\r\n";
        let doc = TextDocument::read(raw).unwrap();
        assert!(!doc.was_pfil);
        assert_eq!(doc.line_ending, "\r\n");

        let pfil = compress_pfil(raw, None).unwrap();
        let doc_pfil = TextDocument::read(&pfil).unwrap();
        assert!(doc_pfil.was_pfil);
        let written = doc_pfil.write_with_lines(&doc_pfil.lines).unwrap();
        assert!(is_pfil(&written));
    }
}
