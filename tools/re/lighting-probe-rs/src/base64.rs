use crate::error::ProbeError;

/// Decodes standard RFC 4648 base64 string, ignoring any whitespace (CR, LF, space, tab).
pub fn decode_base64(input: &str) -> Result<Vec<u8>, ProbeError> {
    let mut clean = Vec::with_capacity(input.len());
    for b in input.bytes() {
        if b.is_ascii_whitespace() {
            continue;
        }
        clean.push(b);
    }

    if clean.is_empty() {
        return Ok(Vec::new());
    }

    if clean.len() % 4 != 0 {
        return Err(ProbeError::Base64(
            "Base64 length must be a multiple of 4".into(),
        ));
    }

    let mut output = Vec::with_capacity((clean.len() / 4) * 3);
    let mut padded = false;
    for chunk in clean.chunks_exact(4) {
        if padded {
            return Err(ProbeError::Base64("Base64 data found after padding".into()));
        }

        let (c0, c1, c2, c3) = (chunk[0], chunk[1], chunk[2], chunk[3]);

        let v0 = decode_char(c0)?;
        let v1 = decode_char(c1)?;

        let (v2, p2) = if c2 == b'=' {
            (0, true)
        } else {
            (decode_char(c2)?, false)
        };
        let (v3, p3) = if c3 == b'=' {
            (0, true)
        } else {
            (decode_char(c3)?, false)
        };

        if p2 && !p3 {
            return Err(ProbeError::Base64("Invalid base64 padding".into()));
        }

        output.push((v0 << 2) | (v1 >> 4));

        if !p2 {
            output.push(((v1 & 0x0F) << 4) | (v2 >> 2));
        }
        if !p3 {
            output.push(((v2 & 0x03) << 6) | v3);
        }

        if p2 || p3 {
            padded = true;
        }
    }

    Ok(output)
}

fn decode_char(b: u8) -> Result<u8, ProbeError> {
    match b {
        b'A'..=b'Z' => Ok(b - b'A'),
        b'a'..=b'z' => Ok(b - b'a' + 26),
        b'0'..=b'9' => Ok(b - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(ProbeError::Base64(format!("Invalid base64 byte: {b}"))),
    }
}
