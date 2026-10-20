use std::collections::BTreeMap;
use winmm::{
    config::{Config, Profile},
    detour::*,
    lde::*,
    pe::*,
    targets::*,
    tracelog,
};
#[derive(Default)]
struct Ini {
    ints: BTreeMap<String, i32>,
    timestamp: Option<String>,
}
impl Profile for Ini {
    fn integer(&self, s: &str, k: &str, d: i32) -> i32 {
        *self.ints.get(&format!("{s}.{k}")).unwrap_or(&d)
    }
    fn string(&self, _: &str, _: &str, d: &str) -> String {
        self.timestamp.clone().unwrap_or_else(|| d.into())
    }
}
fn enabled() -> Config {
    Config {
        enableHooks: true,
        expectedTimeDateStamp: 0x404d1710,
        traceOpcodes: true,
        ..Config::default()
    }
}
#[test]
fn config_missing_retains_every_default() {
    assert_eq!(Config::load(&Ini::default()), Config::default());
}
#[test]
fn config_clamps_and_accepts_nonzero_bools() {
    let mut i = Ini::default();
    i.ints.insert("general.argDumpCount".into(), 99);
    i.ints.insert("general.enableHooks".into(), -1);
    i.ints.insert("hooks.traceOpcodes".into(), 1);
    let c = Config::load(&i);
    assert_eq!(c.argDumpCount, 16);
    assert!(c.enableHooks && c.traceOpcodes);
    i.ints.insert("general.argDumpCount".into(), -2);
    assert_eq!(Config::load(&i).argDumpCount, 0);
}
#[test]
fn timestamp_uses_wcstoul_hex_prefix_rules() {
    for (s, v) in [
        ("404D1710", 0x404d1710),
        (" 0x404D1710 trailing", 0x404d1710),
        ("garbage", 0),
        ("-1", u32::MAX),
        ("100000000", u32::MAX),
    ] {
        assert_eq!(
            Config::load(&Ini {
                timestamp: Some(s.into()),
                ..Ini::default()
            })
            .expectedTimeDateStamp,
            v
        );
    }
}
#[test]
fn safe_default_never_selects_hooks() {
    assert!(selected(&Config::default()).is_empty());
}
#[test]
fn unlocked_build_only_selects_faction_variants() {
    let c = Config {
        enableHooks: true,
        ..Config::default()
    };
    let ts = selected(&c);
    assert_eq!(
        ts.iter().map(|t| t.name).collect::<Vec<_>>(),
        vec!["faction", "factionF"]
    );
}
#[test]
fn every_source_signature_and_prologue_is_valid() {
    let ts = selected(&enabled());
    assert_eq!(ts.len(), 11);
    for t in ts {
        assert_eq!(t.signature.len(), t.mask.len(), "{}", t.name);
        assert!(verify_signature(t.signature, t.signature, t.mask));
        let n = stolen_length(t.signature).unwrap();
        assert!((5..=32).contains(&n));
    }
}
#[test]
fn individual_switches_remove_corresponding_targets() {
    let mut c = enabled();
    c.traceNpcJobs = false;
    c.traceNpcActive = false;
    c.traceVillage = false;
    c.traceCreateUnit = false;
    c.traceNpcQuery = false;
    c.traceLevelInit = false;
    c.traceUnitMax = false;
    c.traceOpcodes = false;
    assert_eq!(selected(&c).len(), 2);
    c.enabled = false;
    assert!(selected(&c).is_empty());
}
#[test]
fn signature_mismatch_wildcards_and_truncation() {
    assert!(verify_signature(&[1, 9], &[1, 2], b"x?"));
    assert!(!verify_signature(&[2, 9], &[1, 2], b"x?"));
    assert!(!verify_signature(&[1], &[1, 2], b"xx"));
    assert!(!verify_signature(&[1], &[1], b"xx"));
}
#[test]
fn relative_instructions_including_prefixed_branches_are_rejected() {
    for bytes in [
        &[0xe8, 0, 0, 0, 0][..],
        &[0x66, 0xe9, 0, 0],
        &[0x2e, 0x75, 0],
        &[0x0f, 0x85, 0, 0, 0, 0],
        &[0xe2, 0],
    ] {
        assert!(is_relative(bytes));
        assert!(stolen_length(bytes).is_err());
    }
    assert!(stolen_length(&[0x90, 0xe8, 0, 0, 0, 0]).is_err());
}
#[test]
fn unknown_truncated_and_overlong_fail_closed() {
    for b in [
        &[][..],
        &[0x0f, 0xff],
        &[0x8b],
        &[0x8b, 0x04],
        &[0x68, 0, 0],
        &[0x66; 16],
    ] {
        assert_eq!(decode_length(b), None);
    }
}
#[test]
fn address_override_sib_and_group_immediates() {
    for (b, n) in [
        (&[0x67, 0x8b, 0x06, 0, 0][..], 5),
        (&[0x67, 0x8b, 0x86, 0, 0], 5),
        (&[0x8b, 0x04, 0x25, 0, 0, 0, 0], 7),
        (&[0xf6, 0xc0, 1], 3),
        (&[0x66, 0xf7, 0xc0, 1, 0], 5),
        (&[0x0f, 0xba, 0xe0, 1], 4),
    ] {
        assert_eq!(decode_length(b), Some(n));
    }
}
fn destination(bytes: &[u8], address: u32) -> u32 {
    address
        .wrapping_add(5)
        .wrapping_add(u32::from_le_bytes(bytes[1..5].try_into().unwrap()))
}
#[test]
fn trampoline_copies_whole_instructions_and_jumps_back() {
    let code = [0x53, 0x56, 0x57, 0x55, 0x8b, 0x5c, 0x24, 0x14];
    let v = trampoline(&code, 0x547f50, 0x70000000).unwrap();
    assert_eq!(&v[..8], &code);
    assert_eq!(destination(&v[8..], 0x70000008), 0x547f58);
}
#[test]
fn patch_pads_tail_and_wraps_rel32() {
    let p = patch(0xfffffff0, 0x100, 8);
    assert_eq!(p[0], 0xe9);
    assert_eq!(destination(&p, 0xfffffff0), 0x100);
    assert_eq!(&p[5..], &[0x90; 3]);
}
#[test]
fn stub_matches_cpp_pushad_flags_and_cdecl_stack_layout() {
    let s = stub(0x1000, 0x12345678, 0x2000, 0x3000);
    assert_eq!(s.len(), 25);
    assert_eq!(&s[..6], &[0x9c, 0x60, 0x89, 0xe0, 0x50, 0x68]);
    assert_eq!(&s[6..10], &0x12345678u32.to_le_bytes());
    assert_eq!(destination(&s[10..15], 0x100a), 0x2000);
    assert_eq!(&s[15..20], &[0x83, 0xc4, 8, 0x61, 0x9d]);
    assert_eq!(destination(&s[20..], 0x1014), 0x3000);
    assert_eq!(std::mem::size_of::<RegBank>(), 32);
}
#[test]
fn pe_headers_are_checked_before_fingerprint() {
    let mut b = vec![0; 256];
    b[..2].copy_from_slice(b"MZ");
    b[0x3c..0x40].copy_from_slice(&64u32.to_le_bytes());
    b[64..68].copy_from_slice(b"PE\0\0");
    b[68..70].copy_from_slice(&[0x4c, 1]);
    b[88..90].copy_from_slice(&[0x0b, 1]);
    for (i, v) in [
        (72, 0x404d1710u32),
        (116, 0x400000),
        (144, 0x3000000),
        (104, 0x1000),
    ] {
        b[i..i + 4].copy_from_slice(&v.to_le_bytes());
    }
    assert_eq!(
        fingerprint(&b),
        Some(Fingerprint {
            timestamp: 0x404d1710,
            preferred_base: 0x400000,
            image_size: 0x3000000,
            entry: 0x1000
        })
    );
    for n in 0..148 {
        assert!(fingerprint(&b[..n]).is_none());
    }
    b[88] = 0x0c;
    assert!(fingerprint(&b).is_none());
}
#[test]
fn timestamp_gate_and_documented_base_relocation() {
    assert!(build_matches(0, 1));
    assert!(build_matches(42, 42));
    assert!(!build_matches(42, 43));
    assert_eq!(rebase(0x45bd60, 0x10000000), 0x1005bd60);
}
struct Missing;
impl Memory for Missing {
    fn read(&self, _: u32, _: &mut [u8]) -> bool {
        false
    }
}
#[test]
fn event_arguments_and_categories_match_cpp() {
    let ts = selected(&enabled());
    let cs = [0x12345678, 5, 1, 4, 0, 0, 20, 20, 8, 3];
    let t = ts.iter().find(|t| t.name == "npcjob").unwrap();
    assert_eq!(
        format_hit(t, &cs, 0x400000, &Missing),
        Some((
            "ai.spawn",
            "addNPCJob_createUnit team=5 mode=1 a3=4 a4=0 a5=0 count=20..20 a8=8 a9=3 ret=12345678"
                .into()
        ))
    );
    for t in ts {
        assert!(
            format_hit(&t, &cs, 0x400000, &Missing).is_some(),
            "{}",
            t.name
        );
        assert!(format_hit(&t, &[], 0x400000, &Missing).is_none());
    }
}
#[test]
fn faulting_memory_fallbacks_match_cpp() {
    let ts = selected(&enabled());
    let t = |name| ts.iter().find(|t| t.name == name).unwrap();
    assert!(format_hit(t("village"), &[0, 1, 2, 3], 0x400000, &Missing)
        .unwrap()
        .1
        .contains("\"?\""));
    assert!(format_hit(t("npcquery"), &[0, 99], 0x400000, &Missing)
        .unwrap()
        .1
        .contains("active=255"));
    assert_eq!(
        format_hit(t("bciop"), &[0, 0x1234], 0x400000, &Missing)
            .unwrap()
            .1,
        "ctx=00001234 pc=FFFFFFFF op=4294967295"
    );
}
#[test]
fn opcode_context_is_argument_one_and_reads_pc_codebase_offsets() {
    struct Mem;
    impl Memory for Mem {
        fn read(&self, a: u32, o: &mut [u8]) -> bool {
            let v: u32 = match a {
                0x1008 => 8,
                0x102c => 0x2000,
                0x2008 => 123,
                _ => return false,
            };
            o.copy_from_slice(&v.to_le_bytes());
            true
        }
    }
    let ts = selected(&enabled());
    let t = ts.iter().find(|t| t.name == "bciop").unwrap();
    assert_eq!(
        format_hit(t, &[99, 0x1000], 0x400000, &Mem).unwrap().1,
        "ctx=00001000 pc=00008 op=123"
    );
}
#[test]
fn logger_bom_line_format_and_truncation() {
    let mut b = Vec::new();
    {
        let mut l = tracelog::TraceLog::new(&mut b).unwrap();
        l.raw("banner").unwrap();
        l.close().unwrap();
    }
    assert_eq!(b, b"\xef\xbb\xbfbanner\n");
    assert_eq!(
        tracelog::line(1.25, 10, "hook", "ok"),
        b"[      1.250][t000a][hook      ] ok\n"
    );
    let long = tracelog::line(0., 0, "x", &"a".repeat(5000));
    assert_eq!(long.len(), 4095);
    assert_eq!(long.last(), Some(&b'\n'));
}
#[test]
fn logger_size_cap_appends_warning_and_suppresses_subsequent_writes() {
    let mut b = Vec::new();
    {
        let mut l = tracelog::TraceLog::with_max_bytes(&mut b, Some(80)).unwrap();
        l.log(1, "test", "first message").unwrap();
        assert!(!l.is_stopped());
        l.log(1, "test", "second message which will trigger cap")
            .unwrap();
        assert!(l.is_stopped());
        let written = l.bytes_written();
        l.log(1, "test", "third message suppressed").unwrap();
        assert_eq!(l.bytes_written(), written);
        l.raw("fourth raw message suppressed").unwrap();
        assert_eq!(l.bytes_written(), written);
    }
    let text = String::from_utf8_lossy(&b);
    assert!(text.contains("first message"));
    assert!(text.contains("log size limit reached, suppressing further trace output"));
    assert!(!text.contains("third message"));
    assert!(!text.contains("fourth raw message"));
}
#[test]
fn logger_close_synced_flushes_then_syncs_writer_and_propagates_errors() {
    let mut b = Vec::new();
    let mut synced = false;
    let mut l = tracelog::TraceLog::new(&mut b).unwrap();
    l.raw("x").unwrap();
    l.close_synced(|w| {
        assert_eq!(w.last(), Some(&10u8));
        synced = true;
        Ok(())
    })
    .unwrap();
    assert!(synced);
    let e = l
        .close_synced(|_| Err(std::io::Error::other("sync failed")))
        .unwrap_err();
    assert_eq!(e.to_string(), "sync failed");
}
#[test]
fn exports_snapshot_matches_original_names_ordinals_and_noname() {
    let ours = include_str!("../winmm.def");
    let source = include_str!("../../argm-trace/argm_trace.def");
    assert_eq!(ours, source);
    let entries: Vec<_> = ours
        .lines()
        .filter(|l| l.trim().starts_with("Thunk_Ordinal2") || l.contains("= Thunk_"))
        .collect();
    assert_eq!(entries.len(), 193);
    assert!(entries[0].contains("@2 NONAME"));
}

#[test]
fn malformed_export_directories_fail_without_panicking() {
    for b in [&[][..], b"MZ", &[0; 64][..]] {
        assert!(exports(b).is_none());
    }
    let mut b = vec![0; 512];
    b[..2].copy_from_slice(b"MZ");
    b[60..64].copy_from_slice(&64u32.to_le_bytes());
    b[64..68].copy_from_slice(b"PE\0\0");
    b[88..90].copy_from_slice(&[0x0b, 1]);
    b[84..86].copy_from_slice(&224u16.to_le_bytes());
    assert_eq!(exports(&b), Some(vec![]));
    b[184..188].copy_from_slice(&0x1000u32.to_le_bytes());
    assert!(exports(&b).is_none()); // RVA has no backing section.
    b[60..64].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(exports(&b).is_none());
}

#[test]
fn negative_timestamp_overflow_saturates_like_windows_wcstoul() {
    let i = Ini {
        timestamp: Some("-100000000".into()),
        ..Ini::default()
    };
    assert_eq!(Config::load(&i).expectedTimeDateStamp, u32::MAX);
}
