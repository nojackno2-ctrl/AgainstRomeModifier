//! .NET 8 字元語意的 Rust 對應：`char.IsLetterOrDigit(char)` 與 `StringComparison.OrdinalIgnoreCase`。
//! 資料表由 `tests/csharp_oracle`（真 .NET 8 執行階段）產生於 `dotnet_unicode_tables.rs`，
//! 不使用 Rust 的 `is_alphabetic`/`to_lowercase`（分類與完整大小寫對映皆與 .NET 不同）。

#[path = "dotnet_unicode_tables.rs"]
mod tables;

use tables::{LETTER_OR_DIGIT_BMP, ORDINAL_IGNORE_CASE_FOLD};

/// `char.IsLetterOrDigit` 作用於單一 UTF-16 code unit（surrogate 一律為 false）。
pub fn is_letter_or_digit_unit(unit: u16) -> bool {
    LETTER_OR_DIGIT_BMP
        .binary_search_by(|&(s, e)| {
            if unit < s {
                std::cmp::Ordering::Greater
            } else if unit > e {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

fn fold_char(c: char) -> char {
    let cp = c as u32;
    match ORDINAL_IGNORE_CASE_FOLD.binary_search_by_key(&cp, |&(from, _)| from) {
        Ok(i) => char::from_u32(ORDINAL_IGNORE_CASE_FOLD[i].1).unwrap_or(c),
        Err(_) => c,
    }
}

/// `string.Equals(a, b, StringComparison.OrdinalIgnoreCase)`：逐碼點簡單大寫對映後比較。
pub fn ordinal_ignore_case_eq(a: &str, b: &str) -> bool {
    a.chars().map(fold_char).eq(b.chars().map(fold_char))
}

/// 供 `HashSet` 使用的 OrdinalIgnoreCase 正規化鍵：兩字串 `ordinal_ignore_case_eq` 當且僅當鍵相同。
pub fn ordinal_ignore_case_key(s: &str) -> String {
    s.chars().map(fold_char).collect()
}
