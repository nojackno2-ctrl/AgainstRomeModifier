//! Word-pattern search and verified patching over decompressed BCI bytes.
//! Port of `BciPattern` (src.Core/Core/Bci/BciPattern.cs). Matching semantics are identical for
//! well-formed input (4-byte stepping, little-endian words, `None` = wildcard). Where C# would
//! throw, loop forever or silently misbehave (empty pattern, unaligned start) this port errors, and
//! every write is verified and applied only after the whole edit has been validated.
use crate::Result;

/// One signature word; `None` matches any value.
pub type WordPattern = [Option<i32>];

const WORD: usize = 4;

fn read_word(data: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(data[offset..offset + WORD].try_into().unwrap())
}

fn pattern_bytes(pattern: &WordPattern) -> Result<usize> {
    if pattern.is_empty() {
        return Err("empty BCI word pattern".into());
    }
    pattern
        .len()
        .checked_mul(WORD)
        .ok_or_else(|| "BCI word pattern length overflow".to_owned())
}

/// First match at or after `start` (stepping by one word). `start` must be word aligned.
pub fn find_word_pattern(
    data: &[u8],
    pattern: &WordPattern,
    start: usize,
) -> Result<Option<usize>> {
    let bytes = pattern_bytes(pattern)?;
    if start % WORD != 0 {
        return Err(format!("unaligned BCI search offset 0x{start:X}"));
    }
    let mut offset = start;
    while offset
        .checked_add(bytes)
        .is_some_and(|end| end <= data.len())
    {
        if pattern
            .iter()
            .enumerate()
            .all(|(i, e)| e.is_none_or(|v| read_word(data, offset + i * WORD) == v))
        {
            return Ok(Some(offset));
        }
        offset += WORD;
    }
    Ok(None)
}

/// Every match, including overlapping ones (C# advances one word after each hit).
pub fn find_all_word_pattern_sites(data: &[u8], pattern: &WordPattern) -> Result<Vec<usize>> {
    let mut sites = vec![];
    let mut next = 0;
    while let Some(site) = find_word_pattern(data, pattern, next)? {
        sites.push(site);
        next = site + WORD;
    }
    Ok(sites)
}

/// The single match, or an error when the signature is absent or ambiguous.
pub fn find_unique_word_pattern_site(
    data: &[u8],
    pattern: &WordPattern,
    patch_name: &str,
) -> Result<usize> {
    let sites = find_all_word_pattern_sites(data, pattern)?;
    match sites.as_slice() {
        [site] => Ok(*site),
        [] => Err(format!("{patch_name}: signature not found")),
        _ => Err(format!(
            "{patch_name}: signature is ambiguous ({} sites)",
            sites.len()
        )),
    }
}

/// `VerifiedBinaryWriter.WriteInt32`: overwrite only when the word currently equals `expected`.
pub fn write_i32_verified(
    buffer: &mut [u8],
    offset: usize,
    expected: i32,
    value: i32,
    patch_name: &str,
) -> Result<()> {
    let end = offset
        .checked_add(WORD)
        .filter(|end| *end <= buffer.len())
        .ok_or_else(|| format!("{patch_name}: offset 0x{offset:X} is outside the BCI"))?;
    let current = read_word(buffer, offset);
    if current != expected {
        return Err(format!(
            "{patch_name}: word at 0x{offset:X} is {current}, expected {expected}; file may be a different version"
        ));
    }
    buffer[offset..end].copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn check_replacement(pattern: &WordPattern, replacement: &WordPattern) -> Result<()> {
    pattern_bytes(pattern)?;
    if replacement.len() != pattern.len() {
        return Err("BCI replacement length differs from pattern length".into());
    }
    Ok(())
}

/// Replaces the unique match. `Some(v)` writes `v`, `None` keeps the word. Returns the site.
/// The buffer is untouched on error.
pub fn replace_unique_word_pattern(
    buffer: &mut [u8],
    pattern: &WordPattern,
    replacement: &WordPattern,
    patch_name: &str,
) -> Result<usize> {
    check_replacement(pattern, replacement)?;
    let site = find_unique_word_pattern_site(buffer, pattern, patch_name)?;
    apply_at(buffer, site, replacement, patch_name)?;
    Ok(site)
}

/// Replaces every match and requires exactly `expected_count` of them. Overlapping sites that
/// would both write are rejected. All-or-nothing: the buffer is untouched on error.
pub fn replace_all_word_pattern_sites(
    buffer: &mut [u8],
    pattern: &WordPattern,
    replacement: &WordPattern,
    expected_count: usize,
    patch_name: &str,
) -> Result<Vec<usize>> {
    check_replacement(pattern, replacement)?;
    let sites = find_all_word_pattern_sites(buffer, pattern)?;
    if sites.len() != expected_count {
        return Err(format!(
            "{patch_name}: found {} sites, expected {expected_count}",
            sites.len()
        ));
    }
    if replacement.iter().any(Option::is_some)
        && sites.windows(2).any(|w| w[1] - w[0] < pattern.len() * WORD)
    {
        return Err(format!("{patch_name}: overlapping signature sites"));
    }
    for &site in &sites {
        apply_at(buffer, site, replacement, patch_name)?;
    }
    Ok(sites)
}

/// Writes the `Some` words at `site`. Bounds are guaranteed by the preceding match, so no write
/// can fail midway.
fn apply_at(
    buffer: &mut [u8],
    site: usize,
    replacement: &WordPattern,
    patch_name: &str,
) -> Result<()> {
    for (i, value) in replacement.iter().enumerate() {
        if let Some(v) = value {
            let offset = site + i * WORD;
            let current = read_word(buffer, offset);
            write_i32_verified(buffer, offset, current, *v, patch_name)?;
        }
    }
    Ok(())
}
