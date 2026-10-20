//! 戰役匯出閉環整合測試：model -> compiler -> SCRIPT/ak_level.bci -> 回讀驗證，
//! 以及各失敗路徑下原檔位元不變、不遺留暫存檔。全部使用合成暫存地圖。

use against_rome_campaign::*;
use against_rome_scenario::{
    Action, BciImage, DataBindingSource, Event, ScenarioDocument, StartCamera,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

struct TempMap(PathBuf);

impl TempMap {
    fn new(script_dir_name: &str, script: Option<&[u8]>) -> Self {
        let root = std::env::temp_dir().join(format!("arm-campaign-export-{}", Uuid::new_v4()));
        let dir = root.join(script_dir_name);
        std::fs::create_dir_all(&dir).unwrap();
        if let Some(bytes) = script {
            std::fs::write(dir.join("ak_level.bci"), bytes).unwrap();
        }
        Self(root)
    }
    fn script_dir(&self) -> PathBuf {
        std::fs::read_dir(&self.0)
            .unwrap()
            .flatten()
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("SCRIPT"))
            .unwrap()
            .path()
    }
    fn script(&self) -> PathBuf {
        self.script_dir().join("ak_level.bci")
    }
    fn backup(&self) -> PathBuf {
        self.script_dir().join("ak_level.arm_original")
    }
}

impl Drop for TempMap {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// SCRIPT 目錄中所有檔案的內容快照，用來證明失敗路徑沒有任何副作用。
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap()))
        .collect()
}

fn pfil_wrap(raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 64];
    out[..4].copy_from_slice(b"PFIL");
    out[4] = 0x5A; // 任意標頭殘留位元組，匯出必須原樣保留
    out[16..20].copy_from_slice(&(raw.len() as u32).to_le_bytes());
    for chunk in raw.chunks(8) {
        out.push(((1u16 << chunk.len()) - 1) as u8);
        out.extend_from_slice(chunk);
    }
    out
}

fn aliases() -> Vec<String> {
    vec!["HOUSE".into(), "GER_INF00".into()]
}

fn cutscene(name: &str, link: Option<&str>) -> CutsceneSequence {
    CutsceneSequence {
        name: name.into(),
        camera_waypoints: vec![
            CameraWaypoint::new([1100.0, 15.0, 2100.0], 30.0, 45.0, 60.0, 0.0, CameraEasingType::Linear, None),
            CameraWaypoint::new([1800.0, 25.0, 2800.0], 25.0, 60.0, 70.0, 8.0, CameraEasingType::SmoothStep, None),
        ],
        subtitles: vec![
            SubtitleKeyframe::new(1.0, 4.0, "Elder", "They are coming from the steppes..."),
            SubtitleKeyframe::new(6.0, 3.0, "", "Hold the line."),
        ],
        unit_orders: vec![UnitOrderKeyframe::new(2.0, "GER_INF00", CutsceneUnitOrderKind::MoveTo, 2000.0, 2500.0)],
        on_complete_trigger_event: link.map(str::to_owned),
        ..Default::default()
    }
}

fn options<'a>(
    scenario: &'a ScenarioDocument,
    cutscenes: &'a [CutsceneSequence],
    aliases: &'a [String],
) -> CampaignPipelineOptions<'a> {
    CampaignPipelineOptions {
        mission_plan: None,
        objective_graph: None,
        cutscenes: Some(cutscenes),
        start_camera: None,
        scenario,
        known_aliases: aliases,
        data_source: None,
    }
}

fn message_event(name: &str, delay: i32, text: &str) -> Event {
    Event {
        name: name.into(),
        delay_seconds: delay,
        actions: vec![Action { kind: 0, text: text.into(), ..Default::default() }],
        ..Default::default()
    }
}

#[test]
fn export_writes_script_and_readback_verifies_raw_bci() {
    let original = BciImage::idle().serialize();
    let map = TempMap::new("SCRIPT", Some(&original));
    let aliases = aliases();
    let scenario = ScenarioDocument::default();
    let cutscenes = [cutscene("Intro", None)];

    let result = CampaignCompiler::export_to_map(options(&scenario, &cutscenes, &aliases), &map.0);
    assert!(result.success, "{:?}", result.diagnostics);
    assert!(result.backup_created);
    assert_eq!(std::fs::read(map.backup()).unwrap(), original, "備份必須是原始位元組");

    let written = std::fs::read(map.script()).unwrap();
    assert_ne!(written, original);
    assert_eq!(result.bytes_written, written.len());

    // 獨立回讀：重新解析磁碟檔並驗證它是原檔的純追加。
    let verification = result.verification.unwrap();
    let again = verify_exported_script(
        &original,
        &written,
        &result.pipeline.as_ref().unwrap().scenario_document,
        Some(&StartCamera::from(result.pipeline.as_ref().unwrap().resolved_camera.as_ref().unwrap())),
    )
    .unwrap();
    assert_eq!(verification, again);
    assert!(verification.patched_original_words <= 2);
    assert!(verification.appended_code_words > 0);
    assert!(verification.messages_found >= 2, "兩則字幕必須進入常數表");
    assert!(verification.aliases_found >= 1, "部隊指令的別名必須進入常數表");
    assert_eq!(verification.native_calls_found, 2);

    // 輸出位元碼可被引擎格式解析，且原始常數表索引未位移。
    let old = BciImage::parse(&original).unwrap();
    let new = BciImage::parse(&written).unwrap();
    assert!(new.constants.starts_with(&old.constants));
    assert!(new.main as usize >= old.code.len() * 4);

    // 不遺留暫存檔。
    let names: Vec<String> = snapshot(&map.script_dir()).into_keys().collect();
    assert_eq!(names, vec!["ak_level.arm_original", "ak_level.bci"]);
}

#[test]
fn export_is_idempotent_and_never_double_injects() {
    let original = BciImage::idle().serialize();
    let map = TempMap::new("SCRIPT", Some(&original));
    let aliases = aliases();
    let scenario = ScenarioDocument::default();
    let cutscenes = [cutscene("Intro", None)];

    let first = CampaignCompiler::export_to_map(options(&scenario, &cutscenes, &aliases), &map.0);
    assert!(first.success, "{:?}", first.diagnostics);
    let after_first = std::fs::read(map.script()).unwrap();

    let second = CampaignCompiler::export_to_map(options(&scenario, &cutscenes, &aliases), &map.0);
    assert!(second.success, "{:?}", second.diagnostics);
    assert!(!second.backup_created);
    assert_eq!(std::fs::read(map.script()).unwrap(), after_first, "重複匯出必須得到相同位元組");
    assert_eq!(std::fs::read(map.backup()).unwrap(), original, "備份不得被覆寫成已注入版本");
}

#[test]
fn export_preserves_pfil_container_and_header() {
    let raw = BciImage::idle().serialize();
    let original = pfil_wrap(&raw);
    let map = TempMap::new("script", Some(&original)); // 小寫目錄名：大小寫不敏感尋址
    let aliases = aliases();
    let scenario = ScenarioDocument::default();
    let cutscenes = [cutscene("Intro", None)];

    let result = CampaignCompiler::export_to_map(options(&scenario, &cutscenes, &aliases), &map.0);
    assert!(result.success, "{:?}", result.diagnostics);
    let written = std::fs::read(map.script()).unwrap();
    assert!(written.starts_with(b"PFIL"));
    assert_eq!(written[4], 0x5A, "PFIL 標頭殘留位元組必須保留");
    assert_eq!(&written[..16], &original[..16]);
    assert_eq!(&written[20..64], &original[20..64]);
    let declared = u32::from_le_bytes(written[16..20].try_into().unwrap()) as usize;
    let decoded = against_rome_data::lzss::decompress_pfil(&written, None).unwrap();
    assert_eq!(decoded.len(), declared);
    BciImage::parse(&decoded).unwrap();
    assert_eq!(std::fs::read(map.backup()).unwrap(), original);
    assert!(result.verification.unwrap().is_pfil);
}

#[test]
fn empty_scenario_round_trips_source_bytes() {
    let original = pfil_wrap(&BciImage::idle().serialize());
    let map = TempMap::new("SCRIPT", Some(&original));
    let aliases = aliases();
    let scenario = ScenarioDocument::default();

    let opts = CampaignPipelineOptions {
        mission_plan: None,
        objective_graph: None,
        cutscenes: None,
        start_camera: None,
        scenario: &scenario,
        known_aliases: &aliases,
        data_source: None,
    };
    let result = CampaignCompiler::export_to_map(opts, &map.0);
    assert!(result.success, "{:?}", result.diagnostics);
    assert_eq!(std::fs::read(map.script()).unwrap(), original);
}

#[test]
fn compile_failure_leaves_every_file_untouched() {
    let original = BciImage::idle().serialize();
    let map = TempMap::new("SCRIPT", Some(&original));
    let aliases = aliases();
    let mut scenario = ScenarioDocument::default();
    for i in 0..255 {
        scenario.events.push(message_event(&format!("E{i}"), 1, "hi"));
    }
    let cutscenes = [cutscene("TooMany", None)]; // 再加 >1 個事件 → 超過 256
    let before = snapshot(&map.script_dir());

    let result = CampaignCompiler::export_to_map(options(&scenario, &cutscenes, &aliases), &map.0);
    assert!(!result.success);
    assert!(result.diagnostics.iter().any(|d| d.contains("256-event limit")));
    assert!(result.script_path.is_none());
    assert_eq!(snapshot(&map.script_dir()), before, "失敗不得建立備份、暫存檔或改動腳本");
}

#[test]
fn missing_script_without_backup_fails_without_creating_files() {
    let map = TempMap::new("SCRIPT", None);
    let aliases = aliases();
    let scenario = ScenarioDocument::default();
    let cutscenes = [cutscene("Intro", None)];

    let result = CampaignCompiler::export_to_map(options(&scenario, &cutscenes, &aliases), &map.0);
    assert!(!result.success);
    assert!(snapshot(&map.script_dir()).is_empty());
}

#[test]
fn corrupt_source_script_is_rejected_and_preserved() {
    let garbage = b"this is not a BCI image".to_vec();
    let map = TempMap::new("SCRIPT", Some(&garbage));
    let aliases = aliases();
    let scenario = ScenarioDocument::default();
    let cutscenes = [cutscene("Intro", None)];

    let result = CampaignCompiler::export_to_map(options(&scenario, &cutscenes, &aliases), &map.0);
    assert!(!result.success);
    assert_eq!(snapshot(&map.script_dir()).len(), 1);
    assert_eq!(std::fs::read(map.script()).unwrap(), garbage);
}

#[test]
fn blocked_temp_path_fails_before_backup_or_replacement() {
    let original = BciImage::idle().serialize();
    let map = TempMap::new("SCRIPT", Some(&original));
    // 暫存檔路徑被目錄佔用 → 無法建立暫存檔
    std::fs::create_dir(map.script_dir().join("ak_level.bci.arm_tmp")).unwrap();
    let aliases = aliases();
    let scenario = ScenarioDocument::default();
    let cutscenes = [cutscene("Intro", None)];

    let result = CampaignCompiler::export_to_map(options(&scenario, &cutscenes, &aliases), &map.0);
    assert!(!result.success);
    assert!(!map.backup().exists(), "暫存失敗時不得留下備份");
    assert_eq!(std::fs::read(map.script()).unwrap(), original);
}

#[test]
fn unreadable_backup_blocks_export_and_keeps_script() {
    let original = BciImage::idle().serialize();
    let map = TempMap::new("SCRIPT", Some(&original));
    std::fs::create_dir(map.backup()).unwrap(); // 備份路徑被目錄佔用 → 無法讀取
    let aliases = aliases();
    let scenario = ScenarioDocument::default();
    let cutscenes = [cutscene("Intro", None)];

    let result = CampaignCompiler::export_to_map(options(&scenario, &cutscenes, &aliases), &map.0);
    assert!(!result.success);
    assert_eq!(std::fs::read(map.script()).unwrap(), original);
    assert!(!map.script_dir().join("ak_level.bci.arm_tmp").exists());
}

#[test]
fn changed_data_file_blocks_export() {
    let original = BciImage::idle().serialize();
    let map = TempMap::new("SCRIPT", Some(&original));
    std::fs::create_dir_all(map.0.join("DATA")).unwrap();
    let data_path = map.0.join("DATA/objects.dat");
    let mut data = vec![0u8; 16 + 100 * 121];
    for (off, v) in [(0, 1u32), (4, 100), (8, 30), (12, 30)] {
        data[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
    std::fs::write(&data_path, &data).unwrap();
    let source = DataBindingSource::load(&map.0).unwrap();
    // 載入後 DATA 被外部修改 → 匯出必須拒絕
    data[16 + 121 * 3] ^= 0xFF;
    std::fs::write(&data_path, &data).unwrap();

    let aliases = aliases();
    let scenario = ScenarioDocument::default();
    let cutscenes = [cutscene("Intro", None)];
    let mut opts = options(&scenario, &cutscenes, &aliases);
    opts.data_source = Some(&source);
    let before = snapshot(&map.script_dir());

    let result = CampaignCompiler::export_to_map(opts, &map.0);
    assert!(!result.success, "{:?}", result.diagnostics);
    assert_eq!(snapshot(&map.script_dir()), before);
}

#[test]
fn dangling_on_complete_link_fails_closed() {
    let original = BciImage::idle().serialize();
    let map = TempMap::new("SCRIPT", Some(&original));
    let aliases = aliases();
    let scenario = ScenarioDocument::default();
    let cutscenes = [cutscene("Intro", Some("NoSuchEvent"))];
    let before = snapshot(&map.script_dir());

    let result = CampaignCompiler::export_to_map(options(&scenario, &cutscenes, &aliases), &map.0);
    assert!(!result.success);
    assert!(result.diagnostics.iter().any(|d| d.contains("NoSuchEvent")));
    assert_eq!(snapshot(&map.script_dir()), before);
}

#[test]
fn resolved_link_to_scenario_event_exports_and_warns_on_delay_mismatch() {
    let original = BciImage::idle().serialize();
    let map = TempMap::new("SCRIPT", Some(&original));
    let aliases = aliases();
    let mut scenario = ScenarioDocument::default();
    scenario.events.push(message_event("StartBattle", 1, "The battle begins!"));
    let cutscenes = [cutscene("Intro", Some("startbattle"))]; // 大小寫不敏感

    let result = CampaignCompiler::export_to_map(options(&scenario, &cutscenes, &aliases), &map.0);
    assert!(result.success, "{:?}", result.diagnostics);
    assert!(
        result.diagnostics.iter().any(|d| d.starts_with("[Warning]") && d.contains("StartBattle")),
        "延遲 1s 與完結時間不同時必須警告: {:?}",
        result.diagnostics
    );
    assert!(result.verification.unwrap().messages_found >= 3);
}

#[test]
fn link_to_event_in_same_pipeline_run_resolves() {
    let aliases = aliases();
    let scenario = ScenarioDocument::default();
    let mut second = cutscene("Second", None);
    second.on_complete_trigger_event = None;
    let first = cutscene("First", Some("Second_字幕_1"));
    let cutscenes = [first, second];

    let result = CampaignCompiler::compile(options(&scenario, &cutscenes, &aliases));
    assert!(result.success, "{:?}", result.diagnostics);
}

#[test]
fn invalid_camera_waypoint_fails_closed() {
    let original = BciImage::idle().serialize();
    let map = TempMap::new("SCRIPT", Some(&original));
    let aliases = aliases();
    let scenario = ScenarioDocument::default();
    let mut seq = cutscene("Intro", None);
    seq.camera_waypoints[0].position = [99999.0, 0.0, 10.0]; // 超出 0..16383
    let cutscenes = [seq];
    let before = snapshot(&map.script_dir());

    let result = CampaignCompiler::export_to_map(options(&scenario, &cutscenes, &aliases), &map.0);
    assert!(!result.success);
    assert!(result.diagnostics.iter().any(|d| d.contains("開局相機無效")));
    assert_eq!(snapshot(&map.script_dir()), before);
}

#[test]
fn explicit_start_camera_overrides_cutscene_and_is_verified() {
    let original = BciImage::idle().serialize();
    let map = TempMap::new("SCRIPT", Some(&original));
    let aliases = aliases();
    let scenario = ScenarioDocument::default();
    let cutscenes = [cutscene("Intro", None)];
    let camera = MapStartCamera { x: 4000.0, z: 5000.0, zoom: 6.0, ..Default::default() };
    let mut opts = options(&scenario, &cutscenes, &aliases);
    opts.start_camera = Some(&camera);

    let result = CampaignCompiler::export_to_map(opts, &map.0);
    assert!(result.success, "{:?}", result.diagnostics);
    let resolved = result.pipeline.unwrap().resolved_camera.unwrap();
    assert_eq!((resolved.x, resolved.z), (4000.0, 5000.0));
    let image = BciImage::parse(&std::fs::read(map.script()).unwrap()).unwrap();
    let names: Vec<Vec<u8>> = (0..image.constant_offsets.len())
        .map(|i| image.constant(i).unwrap().to_vec())
        .collect();
    assert!(names.iter().any(|n| n == b"s_centerToPos"));
    assert!(!names.iter().any(|n| n == b"s_lgcSetEnginePos"), "CenterToPos 不應引入 DirectEnginePos");
}

#[test]
fn verifier_rejects_tampered_outputs() {
    let original = BciImage::idle().serialize();
    let aliases = aliases();
    let scenario = ScenarioDocument::default();
    let cutscenes = [cutscene("Intro", None)];
    let compiled = CampaignCompiler::compile_to_bci(options(&scenario, &cutscenes, &aliases), &original);
    assert!(compiled.success, "{:?}", compiled.diagnostics);
    let doc = &compiled.scenario_document;
    let cam: Option<StartCamera> = compiled.resolved_camera.as_ref().map(Into::into);
    let good = compiled.bci_bytes.clone().unwrap();
    verify_exported_script(&original, &good, doc, cam.as_ref()).unwrap();

    // 1. 輸出等於來源（內容被丟棄）
    assert!(verify_exported_script(&original, &original, doc, cam.as_ref()).is_err());
    // 2. 截斷
    assert!(verify_exported_script(&original, &good[..good.len() - 7], doc, cam.as_ref()).is_err());
    // 3. 改動原始程式碼區超過 2 個字
    let mut image = BciImage::parse(&good).unwrap();
    for w in image.code.iter_mut().take(6) {
        *w = w.wrapping_add(1);
    }
    assert!(verify_exported_script(&original, &image.serialize(), doc, cam.as_ref()).is_err());
    // 4. 缺少訊息常數（改掉其中一個字元）
    let mut image = BciImage::parse(&good).unwrap();
    let at = image
        .constants
        .windows(6)
        .position(|w| w == b"Hold t")
        .expect("字幕應在常數表");
    image.constants[at] = b'X';
    assert!(verify_exported_script(&original, &image.serialize(), doc, cam.as_ref()).is_err());
    // 5. 主入口未指向追加區
    let mut image = BciImage::parse(&good).unwrap();
    image.main = 4;
    assert!(verify_exported_script(&original, &image.serialize(), doc, cam.as_ref()).is_err());
    // 6. PFIL 型態不一致
    assert!(verify_exported_script(&original, &pfil_wrap(&good), doc, cam.as_ref()).is_err());
    // 7. 常數表前綴被位移（插入新的首常數）
    let mut seeded = BciImage::idle();
    seeded.add("keep_me").unwrap();
    let seeded_bytes = seeded.serialize();
    let seeded_out = CampaignCompiler::compile_to_bci(options(&scenario, &cutscenes, &aliases), &seeded_bytes);
    let mut shifted = BciImage::parse(&seeded_out.bci_bytes.unwrap()).unwrap();
    shifted.constants[0] = b'z';
    assert!(verify_exported_script(&seeded_bytes, &shifted.serialize(), doc, cam.as_ref()).is_err());
}
