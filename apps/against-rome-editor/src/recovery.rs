//! 當機復原檔案與背景佇列；所有快照僅寫入編輯器資料目錄。
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(120);
const VERSION: u64 = 1;
const MAX_BYTES: u64 = 128 * 1024 * 1024;

/// Gregorian 曆法換算；快照採 UTC，不依賴本機時區或額外套件。
pub(crate) fn utc_timestamp(seconds: u64) -> String {
    let days = (seconds / 86400) as i128 + 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC", seconds / 3600 % 24, seconds / 60 % 60, seconds % 60)
}

fn map_key(path: &Path) -> String {
    let name = path.to_string_lossy().replace('\\', "/").to_lowercase();
    let hash = name.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}
pub fn snapshot_path(root: &Path, map: &Path) -> PathBuf {
    root.join("recovery")
        .join(map_key(map))
        .join("manifest.json")
}
/// 記錄開圖時的關鍵來源；缺檔也是指紋的一部分，讀取錯誤不得視為缺檔。
pub fn fingerprint(map: &Path) -> Result<Value, String> {
    let mut result = serde_json::Map::new();
    for name in [
        "boden.bmp",
        "collision.bmp",
        "boden.txt",
        "boden.ini",
        "map.ini",
        "map_info.ini",
        "arm_scenario.json",
        "arm_start_camera.json",
        "SCRIPT/ak_level.bci",
        "SCRIPT/ak_level.arm_original",
        "DATA/objects.dat",
        "DATA/objdata.dat",
        "DATA/position.dat",
        "DATA/anim.dat",
        "DATA/gfxtype.dat",
        "DATA/action.dat",
        "DATA/engine.dat",
        "DATA/light.dat",
        "DATA/particle.dat",
        "DATA/lager.dat",
        "DATA/biglager.dat",
        "emboss.bmp",
        "vertex.bmp",
        "smooth.bmp",
    ] {
        let path = map.join(name);
        let value = match std::fs::metadata(&path) {
            Ok(m) => json!([
                m.len(),
                m.modified()
                    .map_err(|e| e.to_string())?
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_nanos()
                    .to_string()
            ]),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Null,
            Err(e) => return Err(e.to_string()),
        };
        result.insert(name.into(), value);
    }
    Ok(Value::Object(result))
}
pub fn manifest(map: &Path, source: &Value, pending: Value) -> Value {
    let mut parts = vec![];
    if !pending["terrain"].is_null() { parts.extend(["terrain_height", "passability"]); }
    if !pending["water"].is_null() { parts.push("water"); }
    if !pending["campaign_camera"].is_null() { parts.push("campaign_camera"); }
    if !pending["materials"].is_null() {
        parts.push("materials");
    }
    if !pending["scenario"].is_null() {
        parts.extend(["scenario", "script_spawns", "start_camera"]);
    }
    json!({"version":VERSION,"map_path":map,"source_fingerprint":source,
        "timestamp":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        "included_parts":parts,"omitted_panels":pending["omitted_panels"], "pending":pending})
}
pub fn read(root: &Path, map: &Path, source: &Value) -> Result<Option<Value>, String> {
    let path = snapshot_path(root, map);
    let metadata = match std::fs::metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if metadata.len() > MAX_BYTES {
        return Err("Snapshot too large / 復原檔過大".into());
    }
    let value: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Corrupt snapshot / 復原檔損毀：{e}"))?;
    if value["version"].as_u64() != Some(VERSION)
        || value["map_path"] != json!(map)
        || !value["pending"].is_object()
        || !value["timestamp"].is_u64()
        || !value["included_parts"].is_array()
    {
        return Err("Unsupported or corrupt snapshot / 復原檔版本不支援或格式損毀".into());
    }
    if value["source_fingerprint"] != *source {
        return Err("Map source changed; restore disabled / 地圖來源已變更，禁止復原".into());
    }
    validate_pending(&value["pending"])?;
    Ok(Some(value))
}
/// 先拒絕缺欄位、尺寸錯誤與未來格式，避免讓損毀資料進入復原工作。
pub(crate) fn validate_pending(p: &Value) -> Result<(), String> {
    let bad = || "Corrupt snapshot payload / 復原內容損毀".to_string();
    let terrain = &p["terrain"];
    for (size, samples) in [
        ("vertex_size", "heights"),
        ("collision_size", "passability"),
    ] {
        let n = terrain[size].as_u64().ok_or_else(bad)?;
        let values: Vec<u8> =
            serde_json::from_value(terrain[samples].clone()).map_err(|_| bad())?;
        if n == 0 || n.checked_mul(n) != Some(values.len() as u64) {
            return Err(bad());
        }
    }
    let (surface, _): (f32, u8) = serde_json::from_value(p["water"].clone()).map_err(|_| bad())?;
    if !surface.is_finite()
        || !p["reset_auxiliary_layers"].is_boolean()
        || !p["camera_pending"].is_boolean()
        || !p["camera_dirty"].is_boolean()
    {
        return Err(bad());
    }
    let _: Vec<String> = serde_json::from_value(p["omitted_panels"].clone()).map_err(|_| bad())?;
    if !p["materials"].is_null() {
        let _: Vec<String> = serde_json::from_value(p["materials"].clone()).map_err(|_| bad())?;
    }
    if !p["scenario"].is_null() {
        let (_, camera): (
            against_rome_scenario::ScenarioDocument,
            against_rome_scenario::StartCamera,
        ) = serde_json::from_value(p["scenario"].clone()).map_err(|_| bad())?;
        camera.validate()?;
    }
    let camera: against_rome_campaign::cinematics::MapStartCamera =
        serde_json::from_value(p["campaign_camera"].clone()).map_err(|_| bad())?;
    camera.validate()?;
    Ok(())
}
enum Command {
    Write(PathBuf, Value, u64),
    Delete(PathBuf, u64),
    #[cfg(test)]
    Barrier(mpsc::Sender<()>),
}
struct Completion {
    path: PathBuf,
    sequence: u64,
    write: bool,
    result: Result<(), String>,
}
pub struct Recovery {
    pub root: Option<PathBuf>,
    pub source: Value,
    pub offer: Option<Result<Value, String>>,
    pub last: Option<Value>,
    pub checked: Instant,
    sender: Option<mpsc::Sender<Command>>,
    worker: Option<std::thread::JoinHandle<()>>,
    cleared: std::collections::HashSet<PathBuf>,
    latest: std::collections::HashMap<PathBuf, u64>,
    sequence: u64,
    pending_error: Option<String>,
    results: Option<mpsc::Receiver<Completion>>,
}
impl Default for Recovery {
    fn default() -> Self {
        Self {
            root: crate::storage::editor_data_dir(),
            source: Value::Null,
            offer: None,
            last: None,
            checked: Instant::now(),
            sender: None,
            worker: None,
            cleared: Default::default(),
            latest: Default::default(),
            sequence: 0,
            pending_error: None,
            results: None,
        }
    }
}
impl Recovery {
    fn send(&mut self, command: Command) -> Result<(), String> {
        if self.sender.is_none() {
            let (tx, rx) = mpsc::channel();
            let (done, results) = mpsc::channel();
            let worker = std::thread::Builder::new()
                .name("editor-recovery".into())
                .spawn(move || {
                    while let Ok(command) = rx.recv() {
                        let completion = match command {
                            Command::Write(path, value, sequence) => Completion {
                                result: write(&path, &value), path, sequence, write: true,
                            },
                            Command::Delete(path, sequence) => Completion {
                                result: delete(&path), path, sequence, write: false,
                            },
                            #[cfg(test)]
                            Command::Barrier(tx) => {
                                let _ = tx.send(());
                                continue;
                            }
                        };
                        let _ = done.send(completion);
                    }
                })
                .map_err(|e| e.to_string())?;
            self.worker = Some(worker);
            self.sender = Some(tx);
            self.results = Some(results);
        }
        self.sender
            .as_ref()
            .ok_or("Recovery worker unavailable")?
            .send(command)
            .map_err(|e| e.to_string())
    }
    #[cfg(test)]
    pub(crate) fn flush(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.send(Command::Barrier(tx)).unwrap();
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
    }
    fn drain_results(&mut self) {
        if let Some(rx) = &self.results {
            while let Ok(completion) = rx.try_recv() {
                let latest = self.latest.get(&completion.path) == Some(&completion.sequence);
                if latest {
                    self.latest.remove(&completion.path);
                    if completion.write && completion.result.is_ok() {
                        self.cleared.remove(&completion.path);
                    }
                }
                if let Err(e) = completion.result {
                    self.last = None;
                    self.pending_error = Some(e);
                }
            }
        }
    }
    pub fn poll(&mut self) -> Option<String> {
        self.drain_results();
        self.pending_error.take()
    }
    pub fn open(&mut self, map: &Path) {
        self.drain_results();
        self.last = None;
        self.checked = Instant::now();
        self.offer = None;
        match fingerprint(map) {
            Ok(source) => {
                self.source = source;
                if self
                    .root
                    .as_ref()
                    .is_some_and(|r| self.cleared.contains(&snapshot_path(r, map)))
                {
                    return;
                }
                self.offer = match self.root.as_ref().map(|r| read(r, map, &self.source)) {
                    Some(Ok(Some(v))) => Some(Ok(v)),
                    Some(Err(e)) => Some(Err(e)),
                    _ => None,
                };
            }
            Err(e) => {
                self.source = Value::Null;
                self.offer = Some(Err(e));
            }
        }
    }
    /// 稍後檢視沿用開圖指紋，不能把外部變更重新當作記憶體工作的基準。
    pub fn review(&mut self, map: &Path) {
        self.drain_results();
        if self
            .root
            .as_ref()
            .is_some_and(|r| self.cleared.contains(&snapshot_path(r, map)))
        {
            self.offer = None;
            return;
        }
        self.offer = match fingerprint(map) {
            Ok(current) if current == self.source => {
                match self.root.as_ref().map(|r| read(r, map, &self.source)) {
                    Some(Ok(Some(value))) => Some(Ok(value)),
                    Some(Err(e)) => Some(Err(e)),
                    _ => None,
                }
            }
            Ok(_) => Some(Err(
                "Map source changed; restore disabled / 地圖來源已變更，禁止復原".into(),
            )),
            Err(e) => Some(Err(e)),
        };
    }
    pub fn save(&mut self, map: &Path, dirty: bool, pending: Value) -> Result<bool, String> {
        if !dirty || self.source.is_null() || self.last.as_ref() == Some(&pending) {
            return Ok(false);
        }
        let root = self
            .root
            .as_ref()
            .ok_or("Editor data directory unavailable / 編輯器資料目錄無法使用")?;
        let path = snapshot_path(root, map);
        self.sequence = self.sequence.wrapping_add(1);
        let sequence = self.sequence;
        self.latest.insert(path.clone(), sequence);
        self.send(Command::Write(
            path.clone(),
            manifest(map, &self.source, pending.clone()),
            sequence,
        ))?;
        self.last = Some(pending);
        Ok(true)
    }
    pub fn clear(&mut self, map: &Path) -> Result<(), String> {
        self.last = None;
        self.offer = None;
        self.delete_snapshot(map)
    }
    /// 刪除其他地圖的快照，不改變目前地圖的復原提示與自動儲存狀態。
    pub(crate) fn delete_snapshot(&mut self, map: &Path) -> Result<(), String> {
        if let Some(root) = &self.root {
            let path = snapshot_path(root, map);
            // 刪除尚在背景排隊時，立即重開也不得重新提示已捨棄的快照。
            self.cleared.insert(path.clone());
            self.sequence = self.sequence.wrapping_add(1);
            let sequence = self.sequence;
            self.latest.insert(path.clone(), sequence);
            self.send(Command::Delete(path, sequence))?;
        }
        Ok(())
    }
}
pub fn write(path: &Path, value: &Value) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().ok_or("Missing recovery directory")?)
        .map_err(|e| e.to_string())?;
    crate::layout_io::write_json_atomic(
        path,
        &serde_json::to_string(value).map_err(|e| e.to_string())?,
    )
}
pub fn delete(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

impl Drop for Recovery {
    fn drop(&mut self) {
        // 正常離開時等候已排入的刪除完成；崩潰則保留最後成功發布的快照。
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_utc_dates_and_actual_included_payloads() {
        assert_eq!(utc_timestamp(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc_timestamp(951827696), "2000-02-29 12:34:56 UTC");
        let empty = manifest(Path::new("map"), &Value::Null, json!({}));
        assert_eq!(empty["included_parts"], json!([]));
        let partial = manifest(Path::new("map"), &Value::Null, json!({"terrain":{},"water":[0,0],"materials":[]}));
        assert_eq!(partial["included_parts"], json!(["terrain_height","passability","water","materials"]));
        let scenario = manifest(Path::new("map"), &Value::Null, json!({"scenario":[{},{}],"campaign_camera":{}}));
        assert_eq!(scenario["included_parts"], json!(["campaign_camera","scenario","script_spawns","start_camera"]));
    }
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("arm-recovery-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn only_dirty_changed_snapshot_and_ordered_discard_deletes() {
        let temp = Temp::new();
        let map = temp.0.join("map");
        std::fs::create_dir(&map).unwrap();
        let mut r = Recovery::default();
        r.root = Some(temp.0.join("editor"));
        r.open(&map);
        let path = snapshot_path(r.root.as_ref().unwrap(), &map);
        assert!(!r.save(&map, false, json!({"a":1})).unwrap());
        assert!(!path.exists());
        assert!(r.save(&map, true, json!({"a":1})).unwrap());
        r.flush();
        assert!(!r.save(&map, true, json!({"a":1})).unwrap());
        assert!(r.save(&map, true, json!({"a":2})).unwrap());
        r.clear(&map).unwrap();
        r.open(&map);
        assert!(r.offer.is_none());
        r.flush();
        assert!(!path.exists());
        assert!(r.poll().is_none());
    }
    #[test]
    fn new_snapshot_after_discard_is_offered_only_after_successful_write() {
        let temp = Temp::new();
        let map = temp.0.join("map");
        std::fs::create_dir(&map).unwrap();
        let mut r = Recovery::default();
        r.root = Some(temp.0.join("editor"));
        r.open(&map);
        let mut pending = json!({"terrain":{"vertex_size":1,"collision_size":1,"heights":[1],"passability":[0]},"water":[0.0,0],"reset_auxiliary_layers":false,"camera_pending":false,"camera_dirty":false,"omitted_panels":[],"campaign_camera":against_rome_campaign::cinematics::MapStartCamera::default()});
        assert!(r.save(&map, true, pending.clone()).unwrap());
        r.flush();
        r.clear(&map).unwrap();
        pending["terrain"]["heights"][0] = json!(2);
        assert!(r.save(&map, true, pending.clone()).unwrap());
        r.open(&map);
        if let Some(Ok(value)) = &r.offer {
            assert_eq!(value["pending"], pending, "不得再次顯示已捨棄的舊快照");
        }
        r.flush();
        r.open(&map);
        assert_eq!(r.offer.as_ref().unwrap().as_ref().unwrap()["pending"], pending);

        pending["terrain"]["heights"][0] = json!(3);
        assert!(r.save(&map, true, pending).unwrap());
        r.clear(&map).unwrap();
        r.flush();
        r.open(&map);
        assert!(r.offer.is_none(), "較舊的寫入不得解除後續捨棄的遮蔽");
        assert!(r.poll().is_none());
    }
    #[test]
    fn failed_new_snapshot_does_not_clear_discard_marker() {
        let temp = Temp::new();
        let blocked = temp.0.join("blocked");
        std::fs::write(&blocked, b"file").unwrap();
        let mut r = Recovery::default();
        r.root = Some(blocked);
        r.open(&temp.0);
        r.clear(&temp.0).unwrap();
        assert!(r.save(&temp.0, true, json!({"a":1})).unwrap());
        r.flush();
        r.open(&temp.0);
        assert!(r.offer.is_none());
        assert!(r.cleared.contains(&snapshot_path(r.root.as_ref().unwrap(), &temp.0)));
        assert!(r.poll().is_some());
        assert!(r.last.is_none());
    }
    #[test]
    fn completing_another_maps_write_does_not_unhide_discarded_snapshot() {
        let temp = Temp::new();
        let map_a = temp.0.join("a");
        let map_b = temp.0.join("b");
        std::fs::create_dir(&map_a).unwrap();
        std::fs::create_dir(&map_b).unwrap();
        let mut r = Recovery::default();
        r.root = Some(temp.0.join("editor"));
        let pending = json!({"terrain":{"vertex_size":1,"collision_size":1,"heights":[1],"passability":[0]},"water":[0.0,0],"reset_auxiliary_layers":false,"camera_pending":false,"camera_dirty":false,"omitted_panels":[],"campaign_camera":against_rome_campaign::cinematics::MapStartCamera::default()});
        r.open(&map_a);
        assert!(r.save(&map_a, true, pending.clone()).unwrap());
        r.clear(&map_a).unwrap();
        r.open(&map_b);
        assert!(r.save(&map_b, true, pending.clone()).unwrap());
        r.flush();
        r.open(&map_a);
        assert!(r.offer.is_none());
        r.open(&map_b);
        assert_eq!(r.offer.as_ref().unwrap().as_ref().unwrap()["pending"], pending);
        assert!(r.poll().is_none());
    }
    #[test]
    fn failed_delete_enqueue_cannot_be_unhidden_by_older_write_completion() {
        let temp = Temp::new();
        let mut r = Recovery::default();
        r.root = Some(temp.0.join("editor"));
        let path = snapshot_path(r.root.as_ref().unwrap(), &temp.0);
        let (commands, command_rx) = mpsc::channel();
        drop(command_rx);
        r.sender = Some(commands);
        let (done, completions) = mpsc::channel();
        r.results = Some(completions);
        r.sequence = 1;
        r.latest.insert(path.clone(), 1);
        done.send(Completion { path: path.clone(), sequence: 1, write: true, result: Ok(()) }).unwrap();
        assert!(r.delete_snapshot(&temp.0).is_err());
        r.open(&temp.0);
        assert!(r.cleared.contains(&path));
        assert!(r.offer.is_none());
    }
    #[test]
    fn atomic_publish_failure_preserves_previous_and_leaves_no_partial() {
        let temp = Temp::new();
        let target = temp.0.join("manifest.json");
        crate::layout_io::write_json_atomic(&target, "old").unwrap();
        assert!(
            crate::layout_io::write_json_atomic_using(&target, "new", |_, _| Err(
                std::io::Error::other("模擬發布失敗")
            ))
            .is_err()
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "old");
        assert_eq!(std::fs::read_dir(&temp.0).unwrap().count(), 1);
        let absent = temp.0.join("absent.json");
        assert!(
            crate::layout_io::write_json_atomic_using(&absent, "new", |_, _| Err(
                std::io::Error::other("模擬發布失敗")
            ))
            .is_err()
        );
        assert!(!absent.exists());
        assert_eq!(std::fs::read_dir(&temp.0).unwrap().count(), 1);
    }
    #[test]
    fn rejects_fingerprint_mismatch_future_and_corrupt_manifest() {
        let temp = Temp::new();
        let map = &temp.0;
        let source = fingerprint(map).unwrap();
        let path = snapshot_path(map, map);
        let pending = json!({"terrain":{"vertex_size":2,"collision_size":1,"heights":[0,0,0,0],"passability":[0]},"water":[0.0,0],"reset_auxiliary_layers":false,"camera_pending":false,"camera_dirty":false,"omitted_panels":[],"campaign_camera":against_rome_campaign::cinematics::MapStartCamera::default()});
        let value = manifest(map, &source, pending);
        write(&path, &value).unwrap();
        assert!(read(map, map, &source).unwrap().is_some());
        std::fs::write(map.join("boden.bmp"), b"changed").unwrap();
        assert!(read(map, map, &fingerprint(map).unwrap())
            .unwrap_err()
            .contains("source changed"));
        let mut future = value;
        future["version"] = json!(999);
        write(&path, &future).unwrap();
        assert!(read(map, map, &source).is_err());
        std::fs::write(&path, b"{broken").unwrap();
        assert!(read(map, map, &source).is_err());
        delete(&path).unwrap();
        assert!(read(map, map, &source).unwrap().is_none());
    }
    #[test]
    fn background_failure_is_reported_and_retry_remains_possible() {
        let temp = Temp::new();
        let blocked = temp.0.join("blocked");
        std::fs::write(&blocked, b"file").unwrap();
        let mut r = Recovery::default();
        r.root = Some(blocked);
        r.open(&temp.0);
        assert!(r.save(&temp.0, true, json!({"a":1})).unwrap());
        r.flush();
        assert!(r.poll().is_some());
        assert!(r.last.is_none());
    }
}
