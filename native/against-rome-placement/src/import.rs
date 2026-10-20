//! Detached cross-store family allocation. Source and destination layouts may differ.
use super::*;
impl ObjectStore {
    pub fn import_family_candidate(
        &self,
        source: &Self,
        slot: usize,
        uid: u32,
        delta: [i32; 3],
        team: u16,
    ) -> Result<(Self, usize, u32)> {
        if team > 8 {
            return Err("invalid scenario team".into());
        }
        let details = source.family_details(slot, uid)?;
        let family = details.object_slots;
        let mut reserved: BTreeSet<_> = (0..self.count)
            .filter(|s| self.active(*s))
            .map(|s| self.col(s, 13))
            .collect();
        let lights = if let Some(b) = self.pools.get("light") {
            Some(LightPool::from_payload(&b.bytes).map_err(|e| e.to_string())?)
        } else {
            None
        };
        if let Some(p) = &lights {
            for l in p.active_snapshots() {
                if l.owner() >= 0 {
                    reserved.insert(l.owner() as usize);
                }
                if l.parent() >= 0 {
                    reserved.insert(l.parent() as usize);
                }
            }
        }
        if let Some(b) = self.pools.get("particle") {
            let p = ParticlePool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
            for s in p.active_slots() {
                let owner = p.capture(s).unwrap().owner();
                if owner >= 0 {
                    reserved.insert(owner as usize);
                }
            }
        }
        if let Some(b) = self.pools.get("lager") {
            for s in 0..LAGER_CAPACITY {
                let o = 16 + s * LAGER_RECORD_SIZE;
                if b.bytes[o] != 0 {
                    for i in 0..10 {
                        let m = i16at(&b.bytes, o + 23 + i * 2);
                        if m >= 0 {
                            reserved.insert(m as usize);
                        }
                    }
                }
            }
        }
        if let Some(b) = self.pools.get("biglager") {
            for s in 0..BIG_LAGER_CAPACITY {
                let o = 12 + s * BIG_LAGER_RECORD_SIZE;
                if b.bytes[o] != 0 {
                    for i in 0..BIG_LAGER_MEMBERS {
                        let m = i16at(&b.bytes, o + 1 + i * 2);
                        if m >= 0 {
                            reserved.insert(m as usize);
                        }
                    }
                }
            }
        }
        fn allocation(
            source: Vec<usize>,
            free: impl Iterator<Item = usize>,
        ) -> Result<BTreeMap<usize, usize>> {
            let targets: Vec<_> = free.take(source.len()).collect();
            if targets.len() != source.len() {
                return Err("insufficient free native pool slots".into());
            }
            Ok(source.into_iter().zip(targets).collect())
        }
        let objects = allocation(
            family.clone(),
            (0..self.count).filter(|s| !self.active(*s) && !reserved.contains(s)),
        )?;
        let all_active: Vec<_> = (0..self.count).filter(|s| self.active(*s)).collect();
        let used_positions: BTreeSet<_> =
            all_active.iter().flat_map(|s| self.poslinks(*s)).collect();
        let mut source_positions = Vec::new();
        for &s in &family {
            for p in [source.link(s, 67), source.link(s, 69), source.col(s, 10)] {
                if p != 65535 && !source_positions.contains(&p) {
                    source_positions.push(p);
                }
            }
        }
        let position = allocation(
            source_positions,
            (0..self.positions).filter(|s| {
                self.pools["position"].bytes[8 + s * 17] == 0 && !used_positions.contains(s)
            }),
        )?;
        let used_data: BTreeSet<_> = all_active.iter().map(|s| self.col(*s, 1)).collect();
        let mut source_data = Vec::new();
        for &s in &family {
            let d = source.col(s, 1);
            if d != 65535 && !source_data.contains(&d) {
                source_data.push(d);
            }
        }
        let data = allocation(
            source_data,
            (0..self.count)
                .filter(|s| self.pools["objdata"].bytes[8 + s * 9] == 0 && !used_data.contains(s)),
        )?;
        let mut aux = BTreeMap::new();
        for (name, o, h, stride) in [
            ("anim", 71, 8, 21),
            ("gfxtype", 73, 8, 15),
            ("action", 77, 12, 25),
        ] {
            let mut source_slots = Vec::new();
            for &s in &family {
                let v = source.link(s, o);
                if v != 65535 && !source_slots.contains(&v) {
                    source_slots.push(v);
                }
            }
            if !source_slots.is_empty() {
                let b = self.pools.get(name).ok_or(format!("missing {name}"))?;
                let used: BTreeSet<_> = all_active.iter().map(|s| self.link(*s, o)).collect();
                aux.insert(
                    name,
                    allocation(
                        source_slots,
                        (0..self.count)
                            .filter(|s| b.bytes[h + s * stride] == 0 && !used.contains(s)),
                    )?,
                );
            }
        }
        let mut used_lights: BTreeSet<_> = all_active.iter().map(|s| self.col(*s, 12)).collect();
        if let Some(b) = self.pools.get("particle") {
            let p = ParticlePool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
            for s in p.active_slots() {
                for ordinal in 0..PARTICLE_CHILDREN as u32 {
                    let child = p
                        .capture(s)
                        .unwrap()
                        .child(ordinal)
                        .map_err(|e| e.to_string())?;
                    if child.active_raw() != 0 && child.light_link() != -1 {
                        used_lights.insert(child.light_link() as usize);
                    }
                }
            }
        }
        if !details.light_slots.is_empty() && !self.pools.contains_key("light") {
            return Err("missing light pool".into());
        }
        let light = allocation(
            details.light_slots.clone(),
            (0..1024).filter(|s| {
                lights
                    .as_ref()
                    .is_none_or(|p| p.capture(*s as u32).is_none())
                    && !used_lights.contains(s)
            }),
        )?;
        let particle = if !details.particle_slots.is_empty() {
            let b = self.pools.get("particle").ok_or("missing particle pool")?;
            let p = ParticlePool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
            let used_particles: BTreeSet<_> = all_active
                .iter()
                .map(|s| self.col(*s, 11))
                .filter(|&x| x != 65535)
                .collect();
            allocation(
                details.particle_slots.clone(),
                (0..PARTICLE_CAPACITY)
                    .filter(|&s| p.capture(s as u32).is_none() && !used_particles.contains(&s)),
            )?
        } else {
            BTreeMap::new()
        };
        let lager = if !details.lager_slots.is_empty() {
            let b = self.pools.get("lager").ok_or("missing lager pool")?;
            let used_lagers: BTreeSet<_> = all_active
                .iter()
                .map(|s| self.col(*s, 5))
                .filter(|&x| x != 65535)
                .collect();
            allocation(
                details.lager_slots.clone(),
                (0..LAGER_CAPACITY).filter(|&s| {
                    b.bytes[16 + s * LAGER_RECORD_SIZE] == 0 && !used_lagers.contains(&s)
                }),
            )?
        } else {
            BTreeMap::new()
        };
        let biglager = if !details.big_lager_slots.is_empty() {
            let b = self.pools.get("biglager").ok_or("missing biglager pool")?;
            let mut used_big = BTreeSet::new();
            if let Some(l) = self.pools.get("lager") {
                for s in 0..LAGER_CAPACITY {
                    if l.bytes[16 + s * LAGER_RECORD_SIZE] != 0 {
                        let bl = i16at(&l.bytes, 16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + s * 2);
                        if bl != -1 {
                            used_big.insert(bl as usize);
                        }
                    }
                }
            }
            allocation(
                details.big_lager_slots.clone(),
                (0..BIG_LAGER_CAPACITY).filter(|&s| {
                    b.bytes[12 + s * BIG_LAGER_RECORD_SIZE] == 0 && !used_big.contains(&s)
                }),
            )?
        } else {
            BTreeMap::new()
        };
        let active_max = all_active
            .iter()
            .map(|s| u32at(&self.pools["objects"].bytes, self.record(*s) + 3) & 0xFFFFFF)
            .max()
            .unwrap_or(0);
        let persisted = self
            .pools
            .get("engine")
            .filter(|b| b.bytes.len() >= 38)
            .map(|b| u32at(&b.bytes, 34))
            .unwrap_or(1);
        let next = (active_max + 1).max(persisted);
        if next == 0 || next as u64 + family.len() as u64 > 0x1000000 {
            return Err("24-bit UID exhausted".into());
        }
        let mut candidate = self.clone();
        let snapshot = source.snapshot();
        fn remap(v: usize, map: &BTreeMap<usize, usize>) -> Result<usize> {
            if v == 65535 {
                Ok(v)
            } else {
                map.get(&v)
                    .copied()
                    .ok_or_else(|| "missing family mapping".into())
            }
        }
        for (&src, &dst) in &position {
            candidate.pools.get_mut("position").unwrap().bytes[8 + dst * 17..25 + dst * 17]
                .copy_from_slice(&snapshot["position"][8 + src * 17..25 + src * 17]);
        }
        for (&src, &dst) in &data {
            for (seg, w) in SEGMENTS.iter().enumerate() {
                let base = 8 + self.count * SEGMENTS[..seg].iter().sum::<usize>();
                let from_base = 8 + source.count * SEGMENTS[..seg].iter().sum::<usize>();
                candidate.pools.get_mut("objdata").unwrap().bytes
                    [base + dst * w..base + (dst + 1) * w]
                    .copy_from_slice(
                        &snapshot["objdata"][from_base + src * w..from_base + (src + 1) * w],
                    );
            }
        }
        for (name, h, stride, tails) in [
            ("anim", 8, 21, 2),
            ("gfxtype", 8, 15, 1),
            ("action", 12, 25, 0),
        ] {
            if let Some(map) = aux.get(name) {
                for (&src, &dst) in map {
                    let b = &mut candidate.pools.get_mut(name).unwrap().bytes;
                    b[h + dst * stride..h + (dst + 1) * stride]
                        .copy_from_slice(&snapshot[name][h + src * stride..h + (src + 1) * stride]);
                    for t in 0..tails {
                        let base = h + self.count * stride + t * self.count * 2;
                        let from_base = h + source.count * stride + t * source.count * 2;
                        b[base + dst * 2..base + dst * 2 + 2].copy_from_slice(
                            &snapshot[name][from_base + src * 2..from_base + src * 2 + 2],
                        );
                    }
                }
            }
        }
        for (index, (&src, &dst)) in objects.iter().enumerate() {
            let r = self.record(dst);
            let from = source.record(src);
            let b = &mut candidate.pools.get_mut("objects").unwrap().bytes;
            b[r..r + 79].copy_from_slice(&snapshot["objects"][from..from + 79]);
            b[r + 3..r + 7].copy_from_slice(&(next + index as u32).to_le_bytes());
            for (o, map) in [(67, &position), (69, &position)] {
                let v = remap(source.link(src, o), map)?;
                candidate.pools.get_mut("objects").unwrap().bytes[r + o..r + o + 2]
                    .copy_from_slice(&(v as u16).to_le_bytes());
            }
            for (name, o) in [("anim", 71), ("gfxtype", 73), ("action", 77)] {
                let v = source.link(src, o);
                if v != 65535 {
                    let v = remap(v, &aux[name])?;
                    candidate.pools.get_mut("objects").unwrap().bytes[r + o..r + o + 2]
                        .copy_from_slice(&(v as u16).to_le_bytes());
                }
            }
            for (c, &col_w) in COLUMNS.iter().enumerate() {
                let v = source.col(src, c);
                let value = match c {
                    1 => remap(v, &data)?,
                    5 => remap(v, &lager)?,
                    10 => remap(v, &position)?,
                    11 => remap(v, &particle)?,
                    12 => remap(v, &light)?,
                    13 => remap(v, &objects)?,
                    _ => v,
                } as u32;
                let o = self.column_offset(dst, c);
                candidate.pools.get_mut("objects").unwrap().bytes[o..o + col_w]
                    .copy_from_slice(&value.to_le_bytes()[..col_w]);
            }
        }
        for (&src, &dst) in &light {
            let mut base = 8;
            for (c, w) in against_rome_data::pool::WIDTHS.iter().enumerate() {
                let o = base + dst * w;
                let source = base + src * w;
                let b = &mut candidate.pools.get_mut("light").unwrap().bytes;
                b[o..o + w].copy_from_slice(&snapshot["light"][source..source + w]);
                if c == 13 || c == 14 {
                    let raw_link = u16at(&snapshot["light"], source);
                    let v = if raw_link != 65535 {
                        remap(raw_link as usize, &objects)?
                    } else {
                        65535
                    };
                    b[o..o + 2].copy_from_slice(&(v as u16).to_le_bytes());
                }
                base += 1024 * w;
            }
        }
        for (&src, &dst) in &particle {
            let src_off = 12 + src * PARTICLE_RECORD_SIZE;
            let dst_off = 12 + dst * PARTICLE_RECORD_SIZE;
            let b = candidate.pools.get_mut("particle").unwrap();
            b.bytes[dst_off..dst_off + PARTICLE_RECORD_SIZE]
                .copy_from_slice(&snapshot["particle"][src_off..src_off + PARTICLE_RECORD_SIZE]);
            let src_owner = i16at(&snapshot["particle"], src_off + 6);
            if src_owner >= 0 {
                let dst_owner = remap(src_owner as usize, &objects)?;
                b.bytes[dst_off + 6..dst_off + 8]
                    .copy_from_slice(&(dst_owner as i16).to_le_bytes());
            }
            for i in 0..PARTICLE_CHILDREN {
                let child_off = dst_off + 12 + i * 35;
                let src_light = i16at(&snapshot["particle"], src_off + 12 + i * 35 + 33);
                if src_light != -1 {
                    let dst_light = remap(src_light as usize, &light)?;
                    b.bytes[child_off + 33..child_off + 35]
                        .copy_from_slice(&(dst_light as i16).to_le_bytes());
                }
            }
            for col in 0..4 {
                let src_state = 12
                    + PARTICLE_CAPACITY * PARTICLE_RECORD_SIZE
                    + col * PARTICLE_CAPACITY * 4
                    + src * 4;
                let dst_state = 12
                    + PARTICLE_CAPACITY * PARTICLE_RECORD_SIZE
                    + col * PARTICLE_CAPACITY * 4
                    + dst * 4;
                b.bytes[dst_state..dst_state + 4]
                    .copy_from_slice(&snapshot["particle"][src_state..src_state + 4]);
            }
        }
        for (&src, &dst) in &biglager {
            let src_off = 12 + src * BIG_LAGER_RECORD_SIZE;
            let dst_off = 12 + dst * BIG_LAGER_RECORD_SIZE;
            let b = candidate.pools.get_mut("biglager").unwrap();
            b.bytes[dst_off..dst_off + BIG_LAGER_RECORD_SIZE]
                .copy_from_slice(&snapshot["biglager"][src_off..src_off + BIG_LAGER_RECORD_SIZE]);
            for i in 0..BIG_LAGER_MEMBERS {
                let off = dst_off + 1 + i * 2;
                let src_m = i16at(&snapshot["biglager"], src_off + 1 + i * 2);
                if src_m != -1 {
                    let dst_m = remap(src_m as usize, &objects)?;
                    b.bytes[off..off + 2].copy_from_slice(&(dst_m as i16).to_le_bytes());
                }
            }
        }
        for (&src, &dst) in &lager {
            let src_off = 16 + src * LAGER_RECORD_SIZE;
            let dst_off = 16 + dst * LAGER_RECORD_SIZE;
            let b = candidate.pools.get_mut("lager").unwrap();
            b.bytes[dst_off..dst_off + LAGER_RECORD_SIZE]
                .copy_from_slice(&snapshot["lager"][src_off..src_off + LAGER_RECORD_SIZE]);
            for i in 0..10 {
                let off = dst_off + 23 + i * 2;
                let src_m = i16at(&snapshot["lager"], src_off + 23 + i * 2);
                if src_m != -1 {
                    let dst_m = remap(src_m as usize, &objects)?;
                    b.bytes[off..off + 2].copy_from_slice(&(dst_m as i16).to_le_bytes());
                }
            }
            let src_big = i16at(
                &snapshot["lager"],
                16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + src * 2,
            );
            let dst_big = if src_big != -1 {
                remap(src_big as usize, &biglager)? as i16
            } else {
                -1
            };
            b.bytes[16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + dst * 2
                ..16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + dst * 2 + 2]
                .copy_from_slice(&dst_big.to_le_bytes());
        }
        if let Some(b) = candidate.pools.get_mut("engine") {
            if b.bytes.len() < 38 {
                return Err("engine missing UID counter".into());
            }
            b.bytes[34..38].copy_from_slice(&(next + family.len() as u32).to_le_bytes());
        }
        let root = objects[&slot];
        let new_uid = u32at(
            &candidate.pools["objects"].bytes,
            candidate.record(root) + 3,
        );
        candidate.translate_family_fixed(root, new_uid, delta)?;
        candidate.set_family_team(root, new_uid, team)?;
        candidate.history.clear();
        candidate.redo.clear();
        Ok((candidate, root, new_uid))
    }
}

impl ObjectStore {
    /// Native editor radians converted from saved degrees exactly as the managed reader.
    pub fn native_rotation(&self, slot: usize, uid: u32) -> Result<f32> {
        self.family_details(slot, uid)?;
        let p = self.link(slot, 67);
        Ok((f32at(&self.pools["position"].bytes, 21 + p * 17) as f64
            * (std::f64::consts::PI / 180.)) as f32)
    }
    /// A detached, safe singleton template (including aliased positions and direct light).
    pub fn singleton_template(&self, slot: usize, uid: u32) -> Result<Self> {
        let f = self.family_details(slot, uid)?;
        if f.object_slots.len() != 1
            || !f.particle_slots.is_empty()
            || !f.lager_slots.is_empty()
            || !f.big_lager_slots.is_empty()
        {
            return Err("linked object is not a safe singleton template".into());
        }
        Ok(self.clone())
    }
    pub fn add_singleton_candidate(
        &self,
        source: &Self,
        slot: usize,
        uid: u32,
        position: [f32; 3],
        radians: f32,
        team: u16,
    ) -> Result<(Self, usize, u32)> {
        source.singleton_template(slot, uid)?;
        if !radians.is_finite() {
            return Err("nonfinite native rotation".into());
        }
        let obj = source
            .objects()
            .into_iter()
            .find(|o| o.slot == slot)
            .ok_or("missing template")?;
        if position.iter().any(|v| !v.is_finite()) {
            return Err("nonfinite singleton position".into());
        }
        // Managed Add allocates distinct p0/p1/(optional p2), even for aliased template links.
        let mut template = source.clone();
        let old = [
            source.link(slot, 67),
            source.link(slot, 69),
            source.col(slot, 10),
        ];
        let records: Vec<_> = old
            .iter()
            .filter(|&&p| p != 65535)
            .map(|&p| source.pools["position"].bytes[8 + p * 17..25 + p * 17].to_vec())
            .collect();
        let first = template.positions;
        for rec in records {
            template
                .pools
                .get_mut("position")
                .unwrap()
                .bytes
                .extend(rec);
            template.positions += 1;
        }
        if template.positions > 65535 {
            return Err("template position capacity exceeded".into());
        }
        template.pools.get_mut("position").unwrap().bytes[4..8]
            .copy_from_slice(&(template.positions as u32).to_le_bytes());
        let rec = template.record(slot);
        for (i, o) in [67, 69].into_iter().enumerate() {
            template.pools.get_mut("objects").unwrap().bytes[rec + o..rec + o + 2]
                .copy_from_slice(&((first + i) as u16).to_le_bytes());
        }
        if old[2] != 65535 {
            let off = template.column_offset(slot, 10);
            template.pools.get_mut("objects").unwrap().bytes[off..off + 2]
                .copy_from_slice(&((first + 2) as u16).to_le_bytes());
        }
        let (mut candidate, root, uid) =
            self.import_family_candidate(&template, slot, uid, [0; 3], team)?;
        let degrees = (radians as f64 * (180. / std::f64::consts::PI)) as f32;
        if !degrees.is_finite() {
            return Err("rotation exceeds native float range".into());
        }
        for p in candidate.poslinks(root) {
            let bytes = &mut candidate.pools.get_mut("position").unwrap().bytes;
            bytes[8 + p * 17] = 1;
            for a in 0..3 {
                bytes[9 + p * 17 + a * 4..13 + p * 17 + a * 4]
                    .copy_from_slice(&position[a].to_le_bytes());
            }
            bytes[21 + p * 17..25 + p * 17].copy_from_slice(&degrees.to_le_bytes());
        }
        for light in candidate.family_details(root, uid)?.light_slots {
            for a in 0..3 {
                let off = 8 + 1024 * 4 * (1 + a) + light * 4;
                let bytes = &mut candidate.pools.get_mut("light").unwrap().bytes;
                let value = f32at(bytes, off) + (position[a] - obj.position[a]);
                if !value.is_finite() {
                    return Err("nonfinite singleton light position".into());
                }
                bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        Ok((candidate, root, uid))
    }
}
/// Signed 16.16 displacements must be exactly representable, never rounded or saturated.
pub fn fixed_displacement(source: [f32; 3], target: [f32; 3]) -> Result<[i32; 3]> {
    let mut out = [0; 3];
    for a in 0..3 {
        let n = (target[a] as f64 - source[a] as f64) * 65536.;
        if !n.is_finite() || n < i32::MIN as f64 || n > i32::MAX as f64 || n.trunc() != n {
            return Err("family displacement exceeds signed16.16 precision/range".into());
        }
        out[a] = n as i32;
    }
    Ok(out)
}
