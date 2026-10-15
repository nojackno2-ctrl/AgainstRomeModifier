//! Opt-in port of EndlessSaveAiRepairService and the enabled P6/P8/P9 migrations.
//! A full save backup is required before using the file-level operation.
use crate::{fail, Result};

const ORIGINAL: &[i32] = &[
    66, 0, 128, 72, 86, 102, 117, 48, 128, 83, 86, 81, 16, 96, 101, 117, 12, 71, 66, 1, 117, 172,
    66, 0, 91, 19, 81, 71, 102, 117, 52, 66, 120000, 66, 60000, 128, 16, 73, -2, 86, 91, 19, 112,
    44, 66, 240000, 66, 120000, 128, 16, 73, -2, 86, 91, 19, 128, 83, 86, 90, 19, 32, 82, 16, 120,
    -9748,
];
const GATE: &[i32] = &[66, 0, 66, 0, 66, 0, 66, 0, 66, 0, 90, 6, 102, 117, 32];
const LEGACY_GATES: &[&[i32]] = &[
    &[112, 272, 66, 0, 66, 0, 66, 0, 66, 0, 90, 6, 102, 117, 32],
    &[
        90, 11, 117, 252, 90, 11, 117, 236, 90, 11, 117, 220, 112, 224, 32,
    ],
    &[
        90, 11, 117, 252, 90, 11, 117, 236, 90, 11, 117, 220, 112, 232, 32,
    ],
];
const HELPER: &[i32] = &[
    74, 94, 73, 3, 66, 0, 90, -4, 90, -3, 128, 86, 73, -3, 86, 71, 90, -5, 90, -4, 90, -3, 128,
    141, 73, -3, 86, 71, 66, 1, 87, 112, 0, 95, 75, 121,
];
fn ultimate() -> Vec<i32> {
    let mut w = vec![
        66, 0, 128, 72, 86, 102, 117, 228, 128, 83, 86, 81, 16, 96, 101, 118, 56, 81, 16, 128, 83,
        86, 66, 60000, 32, 96, 100, 118, 8, 112, 136, 128, 83, 86, 66, 30000, 32, 82, 16,
    ];
    while w.len() < 63 {
        w.extend([112, 0]);
    }
    w.extend([120, -9748]);
    w
}
fn word(b: &[u8], p: usize) -> i32 {
    i32::from_le_bytes(b[p..p + 4].try_into().unwrap())
}
fn write(b: &mut [u8], p: usize, v: i32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}
fn bytes(w: &[i32]) -> Vec<u8> {
    w.iter().flat_map(|v| v.to_le_bytes()).collect()
}
fn pattern(b: &[u8], p: &[Option<i32>]) -> Vec<usize> {
    if b.len() < p.len() * 4 {
        return vec![];
    }
    (0..=b.len() - p.len() * 4)
        .step_by(4)
        .filter(|&off| {
            p.iter()
                .enumerate()
                .all(|(i, v)| v.is_none_or(|v| word(b, off + i * 4) == v))
        })
        .collect()
}
fn exact(b: &[u8], p: usize, w: &[i32]) -> bool {
    b.get(p..p + w.len() * 4).is_some_and(|s| s == bytes(w))
}
fn one(s: Vec<usize>, name: &str) -> Result<usize> {
    if s.len() != 1 {
        return fail(format!(
            "{name}: expected exactly one signature, found {}",
            s.len()
        ));
    }
    Ok(s[0])
}
fn code_end(b: &[u8]) -> Option<usize> {
    if b.len() < 44 || !b.starts_with(b"BCI0") {
        return None;
    }
    let n = word(b, 8);
    if n <= 0 {
        return None;
    }
    let end = 36usize.checked_add(n as usize)?;
    (b.get(end..end + 8) == Some(b"SYMBCONS".as_slice())).then_some(end)
}

struct Sites {
    scheduler: usize,
    loops: Vec<usize>,
    threshold: usize,
    quota: Vec<usize>,
    filter: usize,
    action: usize,
    helper: Option<usize>,
}
fn detect(b: &[u8]) -> Result<Sites> {
    let mut sig: Vec<_> = ORIGINAL.iter().copied().map(Some).collect();
    for i in [32, 34, 45, 47] {
        sig[i] = None;
    }
    let mut sched = pattern(b, &sig);
    sched.extend(pattern(
        b,
        &ultimate().into_iter().map(Some).collect::<Vec<_>>(),
    ));
    let scheduler = one(sched, "P6 scheduler")?;
    let ranges = [
        (960000, 480000),
        (960000, 480000),
        (360000, 240000),
        (120000, 60000),
    ];
    let mut loops = vec![];
    if b.len() >= 24 {
        for p in (0..=b.len() - 24).step_by(4) {
            if (scheduler..scheduler + ORIGINAL.len() * 4).contains(&p) || loops.len() == 4 {
                continue;
            }
            if [word(b, p), word(b, p + 8), word(b, p + 16), word(b, p + 20)] != [66, 66, 128, 16] {
                continue;
            }
            let pair = (word(b, p + 4), word(b, p + 12));
            if [ranges[loops.len()], (30000, 30000), (2000, 1000)].contains(&pair) {
                loops.push(p);
            }
        }
    }
    if loops.len() != 4 {
        return fail("P6 non-scheduler loop count mismatch");
    }
    // C# intentionally uses the first full-length P8 prefix, then checks its gate.
    let mut prefix = vec![
        Some(90),
        Some(0),
        Some(66),
        None,
        Some(96),
        Some(98),
        Some(91),
        Some(11),
    ];
    prefix.resize(23, None);
    let threshold = *pattern(b, &prefix).first().ok_or("P8 signature missing")?;
    if ![4, 8, 30, 40, 70].contains(&word(b, threshold + 12))
        || !(exact(b, threshold + 32, GATE)
            || LEGACY_GATES.iter().any(|w| exact(b, threshold + 32, w)))
    {
        return fail("P8 unknown threshold or control flow");
    }
    let quota = pattern(
        b,
        &[
            Some(81),
            Some(56),
            Some(90),
            Some(-3),
            None,
            None,
            Some(164),
        ],
    );
    if quota.len() != 10 {
        return fail("P9 quota signature count mismatch");
    }
    let q8 = (word(b, quota[8] + 16), word(b, quota[8] + 20));
    let q9 = (word(b, quota[9] + 16), word(b, quota[9] + 20));
    if ![(90, 6), (66, 0)].contains(&q8) || ![(90, 15), (66, 0)].contains(&q9) {
        return fail("P9 unknown quota");
    }
    let filter = *pattern(
        b,
        &[
            Some(128),
            Some(214),
            Some(73),
            Some(-2),
            Some(86),
            Some(66),
            Some(1),
            Some(96),
            Some(102),
            None,
            None,
        ],
    )
    .first()
    .ok_or("P9 filter missing")?;
    if ![(117, 92), (117, 0), (118, 92)].contains(&(word(b, filter + 36), word(b, filter + 40))) {
        return fail("P9 unknown filter");
    }
    let action = one(
        pattern(
            b,
            &[
                Some(90),
                Some(42),
                Some(117),
                Some(56),
                Some(90),
                Some(41),
                Some(90),
                Some(4),
                Some(90),
                Some(3),
                None,
                None,
                None,
                None,
                Some(86),
                Some(71),
                Some(112),
                Some(116),
            ],
        ),
        "P9 action",
    )?;
    let end = code_end(b).unwrap_or(b.len());
    let helper = end
        .checked_sub(HELPER.len() * 4)
        .filter(|&p| exact(b, p, HELPER));
    let call = (word(b, action + 40), word(b, action + 44));
    match helper {
        Some(h)
            if [120, 160].contains(&call.0) && call.1 == h as i32 - (action + 40) as i32 - 8 =>
        {
            ()
        }
        None if call == (128, 141) => (),
        _ => return fail("P9 inconsistent release helper call"),
    }
    Ok(Sites {
        scheduler,
        loops,
        threshold,
        quota,
        filter,
        action,
        helper,
    })
}

/// Returns None for already-repaired bytes, preserving the original compressed file exactly.
/// All signatures are preflighted before any modification; unknown inputs return an error.
pub fn repair_payload(raw: &[u8]) -> Result<Option<Vec<u8>>> {
    let mut b = against_rome_data::lzss::decompress_pfil(raw, None)?;
    let prefix = b"MAPS/ENDL_";
    let suffix = b"/SCRIPT/ak_level";
    let mut found = vec![];
    for p in 0..b.len().saturating_sub(prefix.len() - 1) {
        if !b[p..].starts_with(prefix) {
            continue;
        }
        let d = p + prefix.len();
        if b.get(d..d + 3)
            .is_none_or(|s| !s.iter().all(u8::is_ascii_digit))
            || b.get(d + 3..d + 3 + suffix.len()) != Some(suffix.as_slice())
        {
            continue;
        }
        let end = d + 3 + suffix.len();
        if let Some(i) = b[end..]
            .windows(4)
            .position(|s| s == b"BCI0")
            .filter(|&i| i <= 16)
        {
            found.push(end + i);
        }
    }
    let off = one(found, "endless ak_level BCI")?;
    let mut tail = b[off..].to_vec();
    let s = detect(&tail)?;
    for p in s.loops {
        write(&mut tail, p + 4, 30000);
        write(&mut tail, p + 12, 30000);
    }
    tail[s.scheduler..s.scheduler + ORIGINAL.len() * 4].copy_from_slice(&bytes(&ultimate()));
    write(&mut tail, s.threshold + 12, 70);
    tail[s.threshold + 32..s.threshold + 32 + GATE.len() * 4].copy_from_slice(&bytes(GATE));
    for (p, op, v) in [(s.quota[8], 90, 6), (s.quota[9], 66, 0)] {
        write(&mut tail, p + 16, op);
        write(&mut tail, p + 20, v);
    }
    write(&mut tail, s.filter + 36, 117);
    write(&mut tail, s.filter + 40, 0);
    let helper = if let Some(h) = s.helper {
        h
    } else {
        let end = code_end(&tail).unwrap_or(tail.len());
        let header = code_end(&tail).is_some();
        tail.splice(end..end, bytes(HELPER));
        if header {
            let size = word(&tail, 8);
            write(&mut tail, 8, size + (HELPER.len() * 4) as i32);
        }
        end
    };
    write(&mut tail, s.action + 40, 120);
    write(
        &mut tail,
        s.action + 44,
        helper as i32 - (s.action + 40) as i32 - 8,
    );
    write(&mut tail, s.action + 48, 112);
    write(&mut tail, s.action + 52, 4);
    if tail == b[off..] {
        return Ok(None);
    }
    b.truncate(off);
    b.extend(tail);
    let out = compress_pfil(&b, raw);
    if against_rome_data::lzss::decompress_pfil(&out, None)? != b {
        return fail("recompressed scr.dat failed verification");
    }
    Ok(Some(out))
}

fn compress_pfil(b: &[u8], header: &[u8]) -> Vec<u8> {
    crate::pfil::compress(b, header)
}

#[cfg(test)]
mod tests;
