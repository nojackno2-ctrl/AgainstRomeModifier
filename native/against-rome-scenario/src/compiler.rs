use crate::{model::*, *};
use std::collections::{HashMap, HashSet};
#[derive(Default)]
struct Code {
    start: i32,
    w: Vec<i32>,
}
impl Code {
    fn new(start: i32) -> Self {
        Self { start, w: vec![] }
    }
    fn op(&mut self, o: i32) {
        self.w.push(o)
    }
    fn emit(&mut self, o: i32, v: i32) {
        self.w.extend([o, v])
    }
    fn address(&self) -> i32 {
        self.start + self.w.len() as i32 * 4
    }
    fn jump(&mut self, o: i32, to: i32) {
        self.emit(o, to - self.address() - 8)
    }
    fn placeholder(&mut self, o: i32) -> usize {
        let p = self.w.len();
        self.emit(o, 0);
        p
    }
    fn resolve(&mut self, p: usize) {
        self.w[p + 1] = self.address() - (self.start + p as i32 * 4 + 8)
    }
    fn double(&mut self, v: f64) {
        let b = v.to_bits();
        self.w.extend([67, b as i32, (b >> 32) as i32])
    }
}
fn words(op: i32) -> Option<usize> {
    Some(match op {
        67 => 3,
        64 | 65 | 66 | 68 | 69 | 70 | 73 | 76 | 77 | 78 | 80 | 81 | 82 | 83 | 84 | 90 | 91 | 92
        | 93 | 112 | 113 | 114 | 115 | 116 | 117 | 118 | 120 | 128 | 129 | 160 => 2,
        1 | 2 | 3 | 4 | 5 | 6 | 16 | 17 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42
        | 43 | 44 | 45 | 48 | 49 | 50 | 51 | 52 | 53 | 71 | 72 | 74 | 75 | 85 | 86 | 87 | 88
        | 89 | 94 | 95 | 96 | 97 | 98 | 99 | 100 | 101 | 102 | 103 | 121 | 122 | 123 | 130
        | 131 | 144 | 145 | 161 | 162 | 163 | 164 | 165 | 166 | 176 | 177 => 1,
        _ => return None,
    })
}
/// Rejects ambiguous main loops, unknown opcodes and non-boundary branch targets.
pub fn find_wait_hook(code: &[i32], main: i32) -> Result<usize> {
    if main < 0 || main % 4 != 0 || main as usize / 4 >= code.len() {
        return Err("invalid main".into());
    }
    let main = main as usize / 4;
    let mut inst = HashMap::new();
    let mut at = main;
    while at < code.len() {
        let n = words(code[at]).ok_or_else(|| format!("unknown opcode at {:#x}", at * 4))?;
        if at + n > code.len() {
            return Err("truncated instruction".into());
        }
        inst.insert(at, n);
        at += n;
    }
    let target = |p: usize| -> Result<usize> {
        let t = p as i64 * 4 + 8 + code[p + 1] as i64;
        if t < 0 || t % 4 != 0 || t >= code.len() as i64 * 4 {
            return Err("invalid branch target".into());
        }
        let t = t as usize / 4;
        if t >= main && !inst.contains_key(&t) {
            return Err("branch into operand".into());
        }
        Ok(t)
    };
    for &p in inst.keys() {
        if matches!(code[p], 112..=118 | 120) {
            target(p)?;
        }
    }
    let walk = |start: usize, stop: Option<usize>| -> Result<HashSet<usize>> {
        let mut seen = HashSet::new();
        let mut q = vec![start];
        while let Some(p) = q.pop() {
            if Some(p) == stop {
                seen.insert(p);
                break;
            }
            let Some(&n) = inst.get(&p) else { continue };
            if !seen.insert(p) {
                continue;
            }
            let op = code[p];
            if (112..=118).contains(&op) {
                q.push(target(p)?)
            }
            if op != 112 && !matches!(op, 121..=123) {
                q.push(p + n)
            }
        }
        Ok(seen)
    };
    let reached = walk(main, None)?;
    let mut candidates = vec![];
    for &p in inst.keys() {
        if !reached.contains(&p)
            || p + 2 >= code.len()
            || !((code[p] == 66 && code[p + 1] == 10) || (code[p] == 90 && code[p + 1] >= 0))
            || code[p + 2] != 131
        {
            continue;
        }
        for &b in &reached {
            if code[b] == 112 && b > p {
                let t = target(b)?;
                if t >= main && t <= p && walk(t, Some(p))?.contains(&p) {
                    candidates.push(p);
                    break;
                }
            }
        }
    }
    if candidates.len() != 1 {
        return Err("no unique reachable main-loop wait hook".into());
    }
    Ok(candidates[0])
}
struct Emitter<'a> {
    image: &'a mut BciImage,
    constants: HashMap<String, i32>,
}
impl Emitter<'_> {
    fn constant(&mut self, s: &str) -> Result<i32> {
        if let Some(&i) = self.constants.get(s) {
            return Ok(i);
        }
        let i = self.image.add(s)?;
        self.constants.insert(s.into(), i);
        Ok(i)
    }
    fn call(&mut self, b: &mut Code, name: &str, args: i32, result: bool) -> Result<()> {
        b.emit(128, self.constant(name)?);
        if args > 0 {
            b.emit(73, -args)
        }
        if result {
            b.op(86)
        }
        Ok(())
    }
    fn read(&mut self, b: &mut Code, key: &str) -> Result<()> {
        b.emit(76, self.constant(key)?);
        self.call(b, "s_getScriptVarL", 1, true)
    }
    fn store(&mut self, b: &mut Code, key: &str) -> Result<()> {
        b.emit(76, self.constant(key)?);
        self.call(b, "s_setScriptVarL", 2, false)
    }
    fn now(&mut self, b: &mut Code) -> Result<()> {
        self.call(b, "s_getTime", 0, true)
    }
    fn object(
        &mut self,
        b: &mut Code,
        c: &Condition,
        d: &ScenarioDocument,
        name: &str,
        pos: bool,
    ) -> Result<()> {
        if pos {
            b.emit(78, 1);
            b.emit(78, 0)
        }
        let s = d
            .spawns
            .iter()
            .find(|s| s.id == c.target_id)
            .ok_or("target missing")?;
        if s.prebuilt {
            let slot = d
                .data_slots
                .iter()
                .find(|b| b.spawn_id == s.id)
                .ok_or("binding missing")?;
            b.emit(66, slot.uid as i32);
            b.emit(66, slot.slot + 1)
        } else {
            self.read(b, &runtime_key(s.id, "UID"))?;
            self.read(b, &runtime_key(s.id, "INDEX"))?
        }
        self.call(b, name, if pos { 4 } else { 2 }, true)
    }
}
/// Mutations are transactional: compilation failure leaves the image unchanged.
pub fn inject_events(image: &mut BciImage, d: &ScenarioDocument, original_main: i32) -> Result<()> {
    d.validate(None)?;
    let mut staged = image.clone();
    events_inner(&mut staged, d, original_main)?;
    *image = staged;
    Ok(())
}
fn events_inner(image: &mut BciImage, d: &ScenarioDocument, original_main: i32) -> Result<()> {
    let active: Vec<_> = d.events.iter().filter(|e| e.enabled).collect();
    if active.is_empty() {
        return Ok(());
    }
    let hook = find_wait_hook(&image.code, original_main)?;
    let wait = (image.code[hook], image.code[hook + 1]);
    let continuation = image.main;
    let mut e = Emitter {
        image,
        constants: HashMap::new(),
    };
    let key = |i| format!("ARM_EVENT_DEADLINE_{i}");
    let armed = |i, j| format!("ARM_EVENT_SEEN_{i}_{j}");
    let mut entry = Code::new(e.image.code.len() as i32 * 4);
    for (i, item) in active.iter().enumerate() {
        e.now(&mut entry)?;
        entry.emit(66, item.delay_seconds * 1000);
        entry.op(32);
        e.store(&mut entry, &key(i))?;
        for j in 0..item.conditions.len() {
            entry.emit(66, 0);
            e.store(&mut entry, &armed(i, j))?
        }
    }
    entry.emit(66, 0);
    e.store(&mut entry, "ARM_MISSION_ENDED")?;
    entry.jump(112, continuation);
    let entry_address = e.image.append(&entry.w);
    let mut b = Code::new(e.image.code.len() as i32 * 4);
    b.op(74);
    b.op(94);
    b.emit(
        73,
        2 + active.iter().map(|x| x.conditions.len()).max().unwrap() as i32,
    );
    let mut terminals = vec![];
    e.read(&mut b, "ARM_MISSION_ENDED")?;
    terminals.push(b.placeholder(118));
    for (i, item) in active.iter().enumerate() {
        e.read(&mut b, &key(i))?;
        let done = b.placeholder(113);
        for (j, c) in item.conditions.iter().enumerate() {
            let local = 2 + j as i32;
            if c.kind == 2 {
                b.emit(66, 0);
                b.emit(91, local);
                e.object(&mut b, c, d, "s_getObjPos", true)?;
                b.emit(66, 1);
                b.op(96);
                let failed = b.placeholder(118);
                let mut outside = vec![];
                for (coordinate, limit, min) in [
                    (0, c.min_x, true),
                    (0, c.max_x, false),
                    (1, c.min_z, true),
                    (1, c.max_z, false),
                ] {
                    if min {
                        b.emit(90, coordinate);
                        b.emit(66, limit)
                    } else {
                        b.emit(66, limit);
                        b.emit(90, coordinate)
                    }
                    b.op(96);
                    outside.push(b.placeholder(113))
                }
                b.emit(66, 1);
                b.emit(91, local);
                b.resolve(failed);
                for p in outside {
                    b.resolve(p)
                }
                continue;
            }
            e.object(&mut b, c, d, "s_objExists", false)?;
            b.emit(91, local);
            if c.kind == 0 {
                continue;
            }
            b.emit(90, local);
            let missing = b.placeholder(117);
            b.emit(66, 1);
            e.store(&mut b, &armed(i, j))?;
            e.object(&mut b, c, d, "s_objDead", false)?;
            b.emit(91, local);
            let evaluated = b.placeholder(112);
            b.resolve(missing);
            e.read(&mut b, &armed(i, j))?;
            b.emit(91, local);
            b.resolve(evaluated)
        }
        let mut unmet = vec![];
        for j in 0..item.conditions.len() {
            b.emit(90, 2 + j as i32);
            unmet.push(b.placeholder(117))
        }
        e.now(&mut b)?;
        e.read(&mut b, &key(i))?;
        b.op(96);
        b.op(101);
        let early = b.placeholder(117);
        if item.repeat {
            e.now(&mut b)?;
            b.emit(66, item.delay_seconds * 1000);
            b.op(32)
        } else {
            b.emit(66, -1)
        }
        e.store(&mut b, &key(i))?;
        for a in &item.actions {
            match a.kind {
                0 => {
                    b.emit(76, e.image.game_string(&a.text)?);
                    b.emit(66, 0);
                    e.call(&mut b, "s_showTextBox", 2, false)?
                }
                1 => {
                    b.emit(66, a.hostile as i32);
                    b.emit(66, a.other_team);
                    b.emit(66, a.team);
                    e.call(&mut b, "s_setTeamHostile", 3, false)?
                }
                2 => {
                    if a.move_destination.is_some() {
                        b.emit(66, 0);
                        b.emit(91, 0);
                        b.emit(66, 0);
                        b.emit(91, 1)
                    }
                    for v in [100, 100, 0, a.count] {
                        b.emit(66, v)
                    }
                    b.emit(76, e.constant(&a.alias)?);
                    b.emit(66, a.z.round_ties_even() as i32);
                    b.emit(66, a.x.round_ties_even() as i32);
                    b.emit(66, 0);
                    b.op(16);
                    for v in [1, 0, 1, a.team] {
                        b.emit(66, v)
                    }
                    b.emit(78, 1);
                    b.emit(78, 0);
                    e.call(
                        &mut b,
                        "s_createUnitAndMems",
                        15,
                        a.move_destination.is_some(),
                    )?;
                    if let Some(p) = &a.move_destination {
                        b.emit(66, 1);
                        b.op(96);
                        let failed = b.placeholder(118);
                        for v in [2, 0, 0, p.z, p.x, 8] {
                            b.emit(66, v)
                        }
                        for v in [1, 0, 1, 0] {
                            b.emit(90, v)
                        }
                        e.call(&mut b, "s_sendMsg", 10, false)?;
                        b.resolve(failed)
                    }
                }
                3 | 4 => {
                    b.emit(66, 1);
                    e.store(&mut b, "ARM_MISSION_ENDED")?;
                    b.emit(66, (a.kind == 3) as i32);
                    e.store(&mut b, "GLOBAL_MISSION_RESULT")?;
                    e.call(&mut b, "s_quitGame", 0, false)?;
                    terminals.push(b.placeholder(112))
                }
                _ => return Err("unknown action".into()),
            }
        }
        b.resolve(done);
        b.resolve(early);
        for p in unmet {
            b.resolve(p)
        }
    }
    for p in terminals {
        b.resolve(p)
    }
    b.op(95);
    b.op(75);
    b.emit(wait.0, wait.1);
    b.jump(112, hook as i32 * 4 + 8);
    let poll = e.image.append(&b.w);
    e.image.code[hook] = 112;
    e.image.code[hook + 1] = poll - (hook as i32 * 4 + 8);
    e.image.main = entry_address;
    Ok(())
}
pub fn inject_camera(image: &mut BciImage, c: &StartCamera) -> Result<()> {
    if !c.enabled {
        return Ok(());
    }
    if ![c.x, c.z]
        .iter()
        .all(|v| v.is_finite() && (0. ..=16383.).contains(v))
        || !c.y.is_finite()
        || c.y.abs() > f32::MAX as f64
        || !c.zoom.is_finite()
        || !(0. ..=9.).contains(&c.zoom)
        || !(0..=1).contains(&c.primitive)
    {
        return Err("invalid start camera".into());
    }
    let mut staged = image.clone();
    let mut b = Code::new(staged.code.len() as i32 * 4);
    if c.primitive == 0 {
        b.emit(66, c.z.round_ties_even() as i32);
        b.emit(66, c.x.round_ties_even() as i32);
        b.emit(128, staged.native("s_centerToPos")?);
        b.emit(73, -2)
    } else {
        b.double(c.z);
        b.double(c.y);
        b.double(c.x);
        b.emit(128, staged.native("s_lgcSetEnginePos")?);
        b.emit(73, -6)
    }
    b.double(c.zoom);
    b.emit(128, staged.native("s_lgcSetEngineZoom")?);
    b.emit(73, -2);
    b.jump(112, staged.main);
    staged.main = staged.append(&b.w);
    *image = staged;
    Ok(())
}
/// Generates non-prebuilt starting objects using the original IPR ABI and runtime ID keys.
pub fn inject_spawns(image: &mut BciImage, spawns: &[Spawn]) -> Result<()> {
    let mut staged = image.clone();
    let mut ids = HashSet::new();
    for s in spawns {
        if !s.id.is_nil() && !ids.insert(s.id) {
            return Err("duplicate spawn ID".into());
        }
        if s.alias.trim().is_empty()
            || s.alias.contains('\0')
            || !(0..=20).contains(&s.count)
            || !s.y.is_finite()
            || !(0. ..=16383.).contains(&s.x)
            || !(0. ..=16383.).contains(&s.z)
            || !s.x.is_finite()
            || !s.z.is_finite()
            || if s.count > 0 {
                !(0..=7).contains(&s.team)
            } else {
                !(-1..=15).contains(&s.team)
            }
        {
            return Err("invalid spawn position/team".into());
        }
    }
    let mut b = Code::new(staged.code.len() as i32 * 4);
    let default = staged.add("DEFSCRIPT")?;
    let create = staged.add("s_createObj")?;
    let unit = staged.add("s_createUnitAndMems")?;
    let binding = if spawns.iter().any(|s| !s.id.is_nil()) {
        staged.add("s_setScriptVarL")?
    } else {
        -1
    };
    let mut aliases: HashMap<String, i32> = HashMap::new();
    let store = |b: &mut Code, key: i32| {
        b.emit(76, key);
        b.emit(128, binding);
        b.emit(73, -2);
    };
    b.op(74);
    b.op(94);
    b.emit(73, 2);
    let mut sorted: Vec<_> = spawns.iter().collect();
    sorted.sort_by_key(|s| s.count <= 0);
    let mut waited = false;
    for s in sorted {
        if s.count <= 0 && !waited {
            b.emit(66, 10);
            b.op(131);
            waited = true
        }
        let mut keys = (-1, -1);
        if !s.id.is_nil() {
            keys = (
                staged.add(&runtime_key(s.id, "INDEX"))?,
                staged.add(&runtime_key(s.id, "UID"))?,
            );
            b.emit(66, 0);
            store(&mut b, keys.0);
            b.emit(66, -1);
            store(&mut b, keys.1);
            b.emit(66, 0);
            b.emit(91, 0);
            b.emit(66, -1);
            b.emit(91, 1)
        }
        let alias = if let Some(&v) = aliases.get(&s.alias.to_ascii_lowercase()) {
            v
        } else {
            let v = staged.add(&s.alias)?;
            aliases.insert(s.alias.to_ascii_lowercase(), v);
            v
        };
        let x = s.x.round_ties_even() as i32;
        let z = s.z.round_ties_even() as i32;
        if s.count > 0 {
            for v in [100, 100, 0, s.count] {
                b.emit(66, v)
            }
            b.emit(76, alias);
            b.emit(66, z);
            b.emit(66, x);
            b.emit(66, s.angle.rem_euclid(360));
            b.op(16);
            for v in [1, 0, 1, s.team] {
                b.emit(66, v)
            }
            b.emit(78, 1);
            b.emit(78, 0);
            b.emit(128, unit);
            b.emit(73, -15)
        } else {
            b.emit(76, default);
            for v in [s.team, z, x] {
                b.emit(66, v)
            }
            b.emit(76, alias);
            b.emit(78, 1);
            b.emit(78, 0);
            b.emit(128, create);
            b.emit(73, -7)
        }
        if !s.id.is_nil() {
            b.op(86);
            b.emit(66, 1);
            b.op(96);
            let failed = b.placeholder(118);
            b.emit(90, 1);
            store(&mut b, keys.1);
            b.emit(90, 0);
            store(&mut b, keys.0);
            b.resolve(failed)
        }
    }
    b.op(95);
    b.op(75);
    b.jump(112, staged.main);
    staged.main = staged.append(&b.w);
    *image = staged;
    Ok(())
}
/// Pure bytecode generation from an authoritative original backup.
/// The caller MUST verify prebuilt DATA slot/UID bindings against the source map.
/// This function validates JSON/ABI structure, not external DATA ownership or gameplay.
/// Use compile_script_verified for user-facing exports with a current active-object snapshot.
pub fn compile(
    original: &[u8],
    document: &ScenarioDocument,
    camera: Option<&StartCamera>,
    aliases: Option<&[String]>,
) -> Result<Vec<u8>> {
    document.validate(aliases)?;
    require_alias_catalog(document, aliases)?;
    let raw = if original.starts_with(b"PFIL") {
        against_rome_data::lzss::decompress_pfil(original, None).map_err(|e| e.to_string())?
    } else {
        original.to_vec()
    };
    let mut image = BciImage::parse(&raw)?;
    let original_main = image.main;
    let spawns: Vec<_> = document
        .spawns
        .iter()
        .filter(|s| !s.prebuilt)
        .cloned()
        .collect();
    if !spawns.is_empty() {
        inject_spawns(&mut image, &spawns)?
    }
    inject_events(&mut image, document, original_main)?;
    if let Some(c) = camera {
        inject_camera(&mut image, c)?
    }
    Ok(image.serialize())
}
/// Returns a game-loadable file, retaining PFIL metadata and input compression kind.
/// Literal-only LZSS is deterministic and intentionally favors safety over size.
/// A scenario with no script content restores the original bytes exactly.
pub fn compile_script(
    original: &[u8],
    document: &ScenarioDocument,
    camera: Option<&StartCamera>,
    aliases: Option<&[String]>,
) -> Result<Vec<u8>> {
    document.validate(aliases)?;
    require_alias_catalog(document, aliases)?;
    if let Some(c) = camera {
        c.validate()?;
    }
    let decoded = if original.starts_with(b"PFIL") {
        against_rome_data::lzss::decompress_pfil(original, None).map_err(|e| e.to_string())?
    } else {
        original.to_vec()
    };
    BciImage::parse(&decoded)?;
    let empty = document.spawns.iter().all(|s| s.prebuilt)
        && document.events.iter().all(|e| !e.enabled)
        && camera.is_none_or(|c| !c.enabled);
    if empty {
        return Ok(original.to_vec());
    }
    let raw = compile(original, document, camera, aliases)?;
    if !original.starts_with(b"PFIL") {
        return Ok(raw);
    }
    let mut out = original.get(..64).ok_or("truncated PFIL header")?.to_vec();
    out[16..20].copy_from_slice(&(raw.len() as u32).to_le_bytes());
    for chunk in raw.chunks(8) {
        out.push(((1u16 << chunk.len()) - 1) as u8);
        out.extend(chunk)
    }
    Ok(out)
}

fn require_alias_catalog(document: &ScenarioDocument, aliases: Option<&[String]>) -> Result<()> {
    let needs_aliases = document.spawns.iter().any(|s| !s.prebuilt)
        || document
            .events
            .iter()
            .any(|e| e.actions.iter().any(|a| a.kind == 2));
    if needs_aliases && aliases.is_none_or(|a| a.is_empty()) {
        return Err("script object creation requires a verified ObjDefName alias catalog".into());
    }
    Ok(())
}
/// Checks active object identity, not merely JSON's internal slot/UID consistency.
/// `active` must come from the source map's validated native objects pool.
pub fn verify_binding_snapshot(document: &ScenarioDocument, active: &[(i32, u32)]) -> Result<()> {
    document.validate(None)?;
    for spawn in document.spawns.iter().filter(|s| s.prebuilt) {
        if !document.data_slots.iter().any(|b| b.spawn_id == spawn.id) {
            return Err(format!(
                "prebuilt object {} has no verified DATA binding",
                spawn.id
            ));
        }
    }
    for b in &document.data_slots {
        if b.spawn_id.is_nil() || !active.contains(&(b.slot, b.uid)) {
            return Err(format!(
                "stale or inactive DATA binding: slot {} UID {}",
                b.slot, b.uid
            ));
        }
    }
    Ok(())
}
/// Export boundary: aliases fail closed, and every prebuilt binding needs a current
/// active slot/UID snapshot loaded from the same map. It does not prove gameplay.
pub fn compile_script_verified(
    original: &[u8],
    document: &ScenarioDocument,
    camera: Option<&StartCamera>,
    aliases: Option<&[String]>,
    active_bindings: Option<&[(i32, u32)]>,
) -> Result<Vec<u8>> {
    if !document.data_slots.is_empty() || document.spawns.iter().any(|s| s.prebuilt) {
        verify_binding_snapshot(
            document,
            active_bindings.ok_or("source DATA object snapshot required for prebuilt bindings")?,
        )?;
    }
    compile_script(original, document, camera, aliases)
}
