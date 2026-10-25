//! 唯讀 APT 群組占用清單；保留原值，投影不代表原生尋路判定。
use crate::apt::AptPoint;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FootprintCell {
    pub value: u32,
    pub point: AptPoint,
    pub relative: [i64; 2],
}
#[derive(Clone, Debug)]
pub struct AptFootprint {
    pub version: u32,
    pub origin: AptPoint,
    pub anchor: AptPoint,
    pub groups: Vec<Vec<FootprintCell>>,
}
/// 標準投影；用 i64 避免 signed32 座標相減或乘法溢位。
pub fn project(point: AptPoint, origin: AptPoint) -> [i64; 2] {
    let dx = i64::from(point.x) - i64::from(origin.x);
    let dy = i64::from(point.y) - i64::from(origin.y);
    [
        (32 * dy + 16 * dx).div_euclid(1024),
        (32 * dy - 16 * dx).div_euclid(1024),
    ]
}
impl AptFootprint {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let word = |at: usize| -> Result<u32, String> {
            let b = bytes
                .get(at..at.checked_add(4).ok_or("APT 位移溢位")?)
                .ok_or("APT 占用清單截斷")?;
            Ok(u32::from_le_bytes(
                b.try_into().map_err(|_| "APT word 截斷")?,
            ))
        };
        if bytes.get(..4) != Some(b"APAT") {
            return Err("缺少 APAT 標記".into());
        }
        let version = word(4)?;
        if !(2..=3).contains(&version) {
            return Err("APT 版本必須為 2 或 3".into());
        }
        word(108)?;
        let table = word(12)? as usize;
        if table < 112 {
            return Err("APT header 與 row tables 重疊".into());
        }
        let base = table.checked_add(248).ok_or("APT 位移溢位")?;
        let origin = AptPoint::new(word(base)? as i32, word(base + 4)? as i32);
        let anchor = AptPoint::new(word(base + 8)? as i32, word(base + 12)? as i32);
        let extra = u64::from(word(base + 16)?) + u64::from(word(base + 20)?);
        let groups = word(base + 24)? as usize;
        let variants = word(36)? as usize;
        if !(1..=3).contains(&variants) {
            return Err("APT variant 數量無效".into());
        }
        let lights = if version == 3 { variants } else { 1 };
        let stride = (6 + lights) * 4;
        let headers = base + 28;
        let mut at = (headers as u64) + (groups as u64) * stride as u64 + extra * 8;
        if at > bytes.len() as u64 {
            return Err("APT 群組表截斷".into());
        }
        let anchor_grid = project(anchor, origin);
        let mut result = Vec::with_capacity(groups);
        for group in 0..groups {
            let h = headers + group * stride;
            let n = word(h + 8)? as usize;
            let mut skip = u64::from(word(h + 12)?) * 8;
            for v in 0..lights {
                skip += u64::from(word(h + 16 + v * 4)?) * 8;
            }
            skip += u64::from(word(h + stride - 8)?) * 8 + u64::from(word(h + stride - 4)?) * 16;
            let end = at + n as u64 * 12 + skip;
            if end > bytes.len() as u64 {
                return Err("APT 群組清單截斷".into());
            }
            let mut cells = Vec::with_capacity(n);
            for _ in 0..n {
                let p = at as usize;
                let point = AptPoint::new(word(p + 4)? as i32, word(p + 8)? as i32);
                let grid = project(point, origin);
                cells.push(FootprintCell {
                    value: word(p)?,
                    point,
                    relative: [grid[0] - anchor_grid[0], grid[1] - anchor_grid[1]],
                });
                at += 12;
            }
            at += skip;
            result.push(cells);
        }
        Ok(Self {
            version,
            origin,
            anchor,
            groups: result,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(version: u32) -> Vec<u8> {
        let lights = if version == 3 { 2 } else { 1 };
        let stride = (6 + lights) * 4;
        let mut b = vec![0u8; 388 + 2 * stride + 24];
        fn put(b: &mut [u8], at: usize, v: u32) {
            b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        b[..4].copy_from_slice(b"APAT");
        for (at, v) in [
            (4, version),
            (12, 112),
            (36, 2),
            (368, 1),
            (376, 1),
            (380, 2),
            (384, 2),
        ] {
            put(&mut b, at, v);
        }
        for g in 0..2 {
            let h = 388 + g * stride;
            for off in (8..stride).step_by(4) {
                put(&mut b, h + off, 1);
            }
        }
        for g in 0..2 {
            b.extend_from_slice(&(20u32 + g).to_le_bytes());
            b.extend_from_slice(&(-1i32 - g as i32 * 64).to_le_bytes());
            b.extend_from_slice(&0i32.to_le_bytes());
            b.resize(b.len() + (2 + lights) * 8 + 16, 0xab);
        }
        b
    }
    #[test]
    fn v2_v3_interleaved_lists_and_anchor_floor() {
        for version in [2, 3] {
            let b = fixture(version);
            let doc = AptFootprint::parse(&b).unwrap();
            assert_eq!(doc.version, version);
            assert_eq!(doc.groups.len(), 2);
            assert_eq!(doc.groups[0][0].value, 20);
            assert_eq!(doc.groups[0][0].relative, [-1, 1]);
            assert_eq!(doc.groups[1][0].value, 21);
            assert_eq!(doc.groups[1][0].relative, [-2, 2]);
            for len in 0..b.len() {
                assert!(AptFootprint::parse(&b[..len]).is_err());
            }
        }
    }
    #[test]
    fn negative_rounding_and_extreme_coordinates() {
        assert_eq!(
            project(AptPoint::new(-1, -1), AptPoint::default()),
            [-1, -1]
        );
        assert_eq!(project(AptPoint::new(1, 0), AptPoint::default()), [0, -1]);
        assert_eq!(project(AptPoint::new(-64, 0), AptPoint::default()), [-1, 1]);
        let p = project(
            AptPoint::new(i32::MIN, i32::MAX),
            AptPoint::new(i32::MAX, i32::MIN),
        );
        assert_eq!(p, [67108863, 201326591]);
    }
}
