#![allow(non_upper_case_globals)]
pub trait Memory {
    fn read(&self, address: u32, out: &mut [u8]) -> bool;
}
fn read32(m: &impl Memory, a: u32, fallback: u32) -> u32 {
    let mut b = [0; 4];
    if m.read(a, &mut b) {
        u32::from_le_bytes(b)
    } else {
        fallback
    }
}
fn string(m: &impl Memory, a: u32) -> String {
    let mut s = String::new();
    for i in 0..63 {
        let mut b = [0];
        if !m.read(a.wrapping_add(i), &mut b) {
            return "?".into();
        }
        if b[0] == 0 {
            break;
        }
        s.push(if (0x20..0x7f).contains(&b[0]) {
            b[0] as char
        } else {
            '.'
        });
    }
    s
}
pub fn format_hit(
    t: &Target,
    cs: &[u32],
    base: u32,
    m: &impl Memory,
) -> Option<(&'static str, String)> {
    if cs.len() <= t.argc.max(usize::from(t.name == "bciop")) {
        return None;
    }
    let r = cs[0];
    Some(match t.name {
        "npcjob" => ("ai.spawn", format!("addNPCJob_createUnit team={} mode={} a3={} a4={} a5={} count={}..{} a8={} a9={} ret={r:08X}",cs[1],cs[2],cs[3],cs[4],cs[5],cs[6],cs[7],cs[8],cs[9])),
        "npcactive" => ("ai.active", format!("setNPCActive team={} active={} ret={r:08X}", cs[1], cs[2])),
        "village" => ("ai.village", format!("setVillageTemplate team={} tmpl={:08X} \"{}\" a3={:08X} ret={r:08X}", cs[1], cs[2], string(m,cs[2]), cs[3])),
        "createunit" => ("ai.unit", format!("createUnitAndMems a1={:08X} a2={:08X} a3={:08X} ret={r:08X}",cs[1],cs[2],cs[3])),
        "npcquery" => { let mut s = [255u8;8]; for (i, b) in s.iter_mut().enumerate() { let mut a=[0]; if m.read(crate::pe::rebase(0x029e6000,base).wrapping_add(i as u32), &mut a) { *b=a[0]; } } let active = s.get(cs[1] as usize).copied().unwrap_or(255); ("ai.query", format!("NPCActive team={} -> active={active} | all[t0={} t1={} t2={} t3={} t4={} t5={} t6={} t7={}] ret={r:08X}",cs[1],s[0],s[1],s[2],s[3],s[4],s[5],s[6],s[7])) },
        "levelinit" => ("ai.level", format!("===== level-init: per-team NPC state reset (NEW SESSION BOUNDARY) ret={r:08X} =====")),
        "battlemax" | "civimax" => ("ai.unitmax", format!("create{}UnitsMax req a1={} a2={} a3={} (clamp<=20) ret={r:08X}",if t.name == "battlemax" {"Battle"}else{"Civi"},cs[1],cs[2],cs[3])),
        "faction" => ("game.faction", format!("factionSelect requested={} ret={r:08X}",cs[1])),
        "factionF" => ("game.faction", format!("factionSelect requested={} effective=3 (force-Roman patch) ret={r:08X}",cs[1])),
        "bciop" => { let ctx=cs[1]; let pc=read32(m,ctx.wrapping_add(8),u32::MAX); let code=read32(m,ctx.wrapping_add(0x2c),0); let op=if code==0 {u32::MAX}else{read32(m,code.wrapping_add(pc),u32::MAX)}; ("bci.op",format!("ctx={ctx:08X} pc={pc:05X} op={op}")) },
        _ => return None,
    })
}
use crate::config::Config;
#[derive(Debug)]
pub struct Target {
    pub name: &'static str,
    pub va: u32,
    pub argc: usize,
    pub signature: &'static [u8],
    pub mask: &'static [u8],
}
pub const Faction: (&[u8], &[u8]) = (
    &[
        0x53, 0x8B, 0x5C, 0x24, 0x08, 0x53, 0xE8, 0x25, 0x3B, 0xFE, 0xFF, 0x83, 0xC4, 0x04, 0x53,
        0x89, 0x1D, 0x78, 0x74, 0x73, 0x00,
    ],
    b"xxxxxxx????xxxxxx????",
);
pub const FactionForced: (&[u8], &[u8]) = (
    &[
        0x53, 0x6A, 0x03, 0x5B, 0x90, 0x53, 0xE8, 0x25, 0x3B, 0xFE, 0xFF, 0x83, 0xC4, 0x04, 0x53,
        0x89, 0x1D, 0x78, 0x74, 0x73, 0x00,
    ],
    b"xxxxxxx????xxxxxx????",
);
pub const NpcJob: (&[u8], &[u8]) = (
    &[
        0x53, 0x56, 0x57, 0x55, 0x8B, 0x5C, 0x24, 0x14, 0x8B, 0x7C, 0x24, 0x30, 0x8B, 0x6C, 0x24,
        0x34,
    ],
    b"xxxxxxxxxxxxxxxx",
);
pub const NpcActive: (&[u8], &[u8]) = (
    &[
        0x8B, 0x54, 0x24, 0x04, 0x85, 0xD2, 0x7C, 0x05, 0x83, 0xFA, 0x08, 0x7C, 0x06, 0xB8, 0xFF,
        0xFF, 0xFF, 0xFF, 0xC3,
    ],
    b"xxxxxxxxxxxxxxxxxxx",
);
pub const Village: (&[u8], &[u8]) = (
    &[
        0x53, 0x56, 0x57, 0x55, 0x8B, 0x5C, 0x24, 0x14, 0x8B, 0x6C, 0x24, 0x18, 0x31, 0xFF, 0x85,
        0xED,
    ],
    b"xxxxxxxxxxxxxxxx",
);
pub const CreateUnit: (&[u8], &[u8]) = (
    &[
        0x53, 0x56, 0x57, 0x55, 0x83, 0xEC, 0x30, 0x8B, 0x44, 0x24, 0x44, 0x8B, 0x70, 0x08,
    ],
    b"xxxxxxxxxxxxxx",
);
pub const Bci: (&[u8], &[u8]) = (
    &[
        0x53, 0x56, 0x57, 0x55, 0x89, 0xE5, 0x81, 0xEC, 0xA8, 0x06, 0x00, 0x00, 0x83, 0xE4, 0xF8,
        0x8B, 0x5D, 0x14, 0x8B, 0x53, 0x08, 0x8B, 0x4B, 0x28,
    ],
    b"xxxxxxxxxxxxxxxxxxxxxxxx",
);
pub const NpcQuery: (&[u8], &[u8]) = (
    &[
        0x8B, 0x44, 0x24, 0x04, 0x85, 0xC0, 0x7C, 0x14, 0x83, 0xF8, 0x08, 0x7D, 0x0F, 0x80, 0xB8,
    ],
    b"xxxxxxxxxxxxxxx",
);
pub const LevelInit: (&[u8], &[u8]) = (
    &[
        0x53, 0x56, 0x57, 0x55, 0x83, 0xEC, 0x24, 0xBB, 0x38, 0x91, 0x9C, 0x02,
    ],
    b"xxxxxxxxxxxx",
);
pub const BattleMax: (&[u8], &[u8]) = (
    &[
        0x53, 0x56, 0x57, 0x55, 0x83, 0xEC, 0x0C, 0x8B, 0x54, 0x24, 0x24,
    ],
    b"xxxxxxxxxxx",
);
pub const CiviMax: (&[u8], &[u8]) = (
    &[
        0x53, 0x56, 0x57, 0x55, 0x83, 0xEC, 0x08, 0x8B, 0x6C, 0x24, 0x24,
    ],
    b"xxxxxxxxxxx",
);
pub fn selected(c: &Config) -> Vec<Target> {
    if !c.enabled || !c.enableHooks {
        return vec![];
    }
    let mut v = Vec::new();
    if true {
        v.push(Target {
            name: "faction",
            va: 0x0045BD60,
            argc: 1,
            signature: Faction.0,
            mask: Faction.1,
        });
    }
    if true {
        v.push(Target {
            name: "factionF",
            va: 0x0045BD60,
            argc: 1,
            signature: FactionForced.0,
            mask: FactionForced.1,
        });
    }
    if c.expectedTimeDateStamp != 0 && c.traceNpcJobs {
        v.push(Target {
            name: "npcjob",
            va: 0x00547F50,
            argc: 9,
            signature: NpcJob.0,
            mask: NpcJob.1,
        });
    }
    if c.expectedTimeDateStamp != 0 && c.traceNpcActive {
        v.push(Target {
            name: "npcactive",
            va: 0x00548CE0,
            argc: 2,
            signature: NpcActive.0,
            mask: NpcActive.1,
        });
    }
    if c.expectedTimeDateStamp != 0 && c.traceVillage {
        v.push(Target {
            name: "village",
            va: 0x00549500,
            argc: 3,
            signature: Village.0,
            mask: Village.1,
        });
    }
    if c.expectedTimeDateStamp != 0 && c.traceCreateUnit {
        v.push(Target {
            name: "createunit",
            va: 0x0052A020,
            argc: 3,
            signature: CreateUnit.0,
            mask: CreateUnit.1,
        });
    }
    if c.expectedTimeDateStamp != 0 && c.traceNpcQuery {
        v.push(Target {
            name: "npcquery",
            va: 0x00548D20,
            argc: 1,
            signature: NpcQuery.0,
            mask: NpcQuery.1,
        });
    }
    if c.expectedTimeDateStamp != 0 && c.traceLevelInit {
        v.push(Target {
            name: "levelinit",
            va: 0x0054A070,
            argc: 0,
            signature: LevelInit.0,
            mask: LevelInit.1,
        });
    }
    if c.expectedTimeDateStamp != 0 && c.traceUnitMax {
        v.push(Target {
            name: "battlemax",
            va: 0x005249D0,
            argc: 3,
            signature: BattleMax.0,
            mask: BattleMax.1,
        });
    }
    if c.expectedTimeDateStamp != 0 && c.traceUnitMax {
        v.push(Target {
            name: "civimax",
            va: 0x00524D70,
            argc: 3,
            signature: CiviMax.0,
            mask: CiviMax.1,
        });
    }
    if c.expectedTimeDateStamp != 0 && c.traceOpcodes {
        v.push(Target {
            name: "bciop",
            va: 0x005B1C60,
            argc: 0,
            signature: Bci.0,
            mask: Bci.1,
        });
    }
    v
}
