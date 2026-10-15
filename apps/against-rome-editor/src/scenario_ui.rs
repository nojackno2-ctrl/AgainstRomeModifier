//! Structured scenario designer and safe new-work-copy exporter.
use against_rome_scenario::{
    self as scenario, Action, Condition, Event, ScenarioDocument, Spawn, StartCamera, WorldPosition,
};
use eframe::egui;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
#[derive(Clone)]
struct Source {
    path: PathBuf,
    bytes: Vec<u8>,
}
#[derive(Clone)]
enum Pending {
    Load(PathBuf),
    Close,
}
pub struct ScenarioUi {
    pub open: bool,
    document: ScenarioDocument,
    camera: StartCamera,
    source: Option<Source>,
    watched: Vec<Source>,
    aliases: Vec<String>,
    active_bindings: Option<Vec<(i32,u32)>>,
    load_path: String,
    export_path: String,
    selected: usize,
    baseline: Vec<u8>,
    pending: Option<Pending>,
    report: String,
    error: Option<String>,
}
impl Default for ScenarioUi {
    fn default() -> Self {
        let document = ScenarioDocument::default();
        let camera = StartCamera {
            enabled: false,
            ..Default::default()
        };
        let baseline = state_bytes(&document, &camera).unwrap();
        Self {
            open: false,
            document,
            camera,
            source: None,
            watched: vec![],
            aliases: vec![],
            active_bindings: None,
            load_path: String::new(),
            export_path: String::new(),
            selected: 0,
            baseline,
            pending: None,
            report: String::new(),
            error: None,
        }
    }
}
fn state_bytes(d: &ScenarioDocument, c: &StartCamera) -> Result<Vec<u8>> {
    serde_json::to_vec(&(d, c)).map_err(|e| e.to_string())
}
fn child_case(parent: &Path, name: &str) -> PathBuf {
    fs::read_dir(parent)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|x| x.ok())
        .find(|x| x.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|x| x.path())
        .unwrap_or_else(|| parent.join(name))
}
fn binding_context(map:&Path,document:&ScenarioDocument)->Result<(Option<Vec<(i32,u32)>>,Vec<Source>)> {
    if document.data_slots.is_empty() && document.spawns.iter().all(|s|!s.prebuilt) {return Ok((None,vec![]))}
    let mut watched=vec![];
    for name in ["objects","objdata","position","anim","gfxtype","action","engine","light","particle","lager","biglager"] {
        let path=map.join("DATA").join(format!("{name}.dat"));
        if path.exists() {watched.push(Source{bytes:fs::read(&path).map_err(|e|e.to_string())?,path});}
    }
    let store=against_rome_placement::ObjectStore::load(map).map_err(|e|format!("無法驗證來源 DATA：{e}"))?;
    let active:Vec<_>=store.objects().iter().map(|o|(o.slot as i32,o.uid)).collect();
    scenario::verify_binding_snapshot(document,&active)?;
    for s in &watched {if fs::read(&s.path).map_err(|e|e.to_string())?!=s.bytes{return Err("來源 DATA 在載入期間改變；請重試。".into())}}
    Ok((Some(active),watched))
}
/// Reject every existing reparse-point ancestor before any filesystem mutation.
fn safe_parent(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    for component in absolute.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err("輸出路徑不可含 ..".into());
        }
    }
    for p in absolute.ancestors() {
        if let Ok(m) = fs::symlink_metadata(p) {
            if m.file_type().is_symlink() {
                return Err("不允許 symlink / junction 路徑".into());
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if m.file_attributes() & 0x400 != 0 {
                    return Err("不允許 reparse point / junction 路徑".into());
                }
            }
        }
    }
    let parent = absolute.parent().ok_or("輸出需要父目錄")?;
    let parent = fs::canonicalize(parent).map_err(|e| e.to_string())?;
    let low = parent.to_string_lossy().to_lowercase();
    if low.contains("program files")
        || low.contains("\\against rome\\")
        || low.ends_with("\\against rome")
    {
        return Err("不可輸出至安裝目錄或 Program Files".into());
    }
    Ok(parent.join(absolute.file_name().ok_or("缺少輸出資料夾名稱")?))
}
impl ScenarioUi {
    pub fn dirty(&self) -> bool {
        state_bytes(&self.document, &self.camera).map_or(true, |s| s != self.baseline)
    }
    fn load_now(&mut self, map: &Path) -> Result<()> {
        let script_dir = child_case(map, "SCRIPT");
        let script = child_case(&script_dir, "ak_level.bci");
        let backup = child_case(&script_dir, "ak_level.arm_original");
        let original = if backup.is_file() { backup } else { script };
        let source = Source {
            bytes: fs::read(&original).map_err(|e| format!("{}: {e}", original.display()))?,
            path: original,
        };
        let scenario_path = map.join("arm_scenario.json");
        let camera_path = map.join("arm_start_camera.json");
        let mut watched = vec![source.clone()];
        let document = if scenario_path.exists() {
            let bytes = fs::read(&scenario_path).map_err(|e| e.to_string())?;
            let d = ScenarioDocument::from_json(&bytes)?;
            watched.push(Source {
                path: scenario_path,
                bytes,
            });
            d
        } else {
            ScenarioDocument::default()
        };
        let camera = if camera_path.exists() {
            let bytes = fs::read(&camera_path).map_err(|e| e.to_string())?;
            let c = serde_json::from_slice(bytes.strip_prefix(&[239, 187, 191]).unwrap_or(&bytes))
                .map_err(|e| e.to_string())?;
            watched.push(Source {
                path: camera_path,
                bytes,
            });
            c
        } else {
            StartCamera {
                enabled: false,
                ..Default::default()
            }
        };
        // Parse BCI immediately; unmodified empty documents must not hide a corrupt source.
        let raw = if source.bytes.starts_with(b"PFIL") {
            against_rome_data::lzss::decompress_pfil(&source.bytes, None)
                .map_err(|e| e.to_string())?
        } else {
            source.bytes.clone()
        };
        scenario::BciImage::parse(&raw)?;
        let (active_bindings,data_watched)=binding_context(map,&document)?;
        watched.extend(data_watched);
        let aliases=map.parent().and_then(Path::parent).and_then(|root|scenario::load_aliases(root).ok()).unwrap_or_default();
        self.baseline = state_bytes(&document, &camera)?;
        self.document = document;
        self.camera = camera;
        self.aliases = aliases;
        self.active_bindings=active_bindings;
        self.source = Some(source);
        self.watched = watched;
        self.selected = 0;
        self.error = None;
        self.report = "已載入；來源保持唯讀。輸出會建立新的工作副本。".into();
        Ok(())
    }
    fn request_load(&mut self, path: PathBuf) {
        if self.dirty() {
            self.pending = Some(Pending::Load(path))
        } else if let Err(e) = self.load_now(&path) {
            self.error = Some(e)
        }
    }
    fn preview(&mut self) -> Result<Vec<u8>> {
        self.camera.validate()?;
        for s in &self.watched {if fs::read(&s.path).map_err(|e|e.to_string())?!=s.bytes{return Err(format!("來源已被外部修改：{}；請重新載入。",s.path.display()))}}
        let original = &self
            .source
            .as_ref()
            .ok_or("請先載入含 SCRIPT/ak_level.bci 的地圖")?
            .bytes;
        let bytes = scenario::compile_script_verified(
            original,
            &self.document,
            Some(&self.camera),
            if self.aliases.is_empty() {
                None
            } else {
                Some(&self.aliases)
            },
            self.active_bindings.as_deref(),
        )?;
        let raw = if bytes.starts_with(b"PFIL") {
            against_rome_data::lzss::decompress_pfil(&bytes, None).map_err(|e| e.to_string())?
        } else {
            bytes.clone()
        };
        let bci = scenario::BciImage::parse(&raw)?;
        self.error = None;
        self.report=format!("腳本編譯通過：{} bytes；CODE {} bytes、常數 {}、main {}。使用別名的動作必須通過來源 catalog；預建綁定已核對來源 DATA。這不是遊戲內驗收。",bytes.len(),bci.code.len()*4,bci.constant_offsets.len(),bci.main);
        Ok(bytes)
    }
    fn export_new(&mut self, path: &Path) -> Result<()> {
        let script = self.preview()?;
        for s in &self.watched {
            if fs::read(&s.path).map_err(|e| e.to_string())? != s.bytes {
                return Err(format!(
                    "來源已被外部修改：{}；請重新載入。",
                    s.path.display()
                ));
            }
        }
        let target = safe_parent(path)?;
        if target.exists() {
            return Err("輸出資料夾已存在；請指定全新的工作副本路徑。".into());
        }
        let mut d = self.document.clone();
        d.version = 6;
        let json = serde_json::to_vec_pretty(&d).map_err(|e| e.to_string())?;
        let camera = serde_json::to_vec_pretty(&self.camera).map_err(|e| e.to_string())?;
        let original = self.source.as_ref().ok_or("missing source")?.bytes.clone();
        export_files(
            &target,
            &[
                ("arm_scenario.json", json),
                ("arm_start_camera.json", camera),
                ("SCRIPT/ak_level.arm_original", original),
                ("SCRIPT/ak_level.bci", script),
            ],
            None,
        )?;
        self.document = d;
        self.baseline = state_bytes(&self.document, &self.camera)?;
        self.report=format!("已輸出劇本工作副本：{}。含原版備份與 JSON；這不是完整地圖封裝，遊戲驗收前需複製其他地圖檔案。",target.display());
        Ok(())
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut opened = true;
        egui::Window::new("劇本事件 / Scenario")
            .open(&mut opened)
            .default_width(760.)
            .show(ctx, |ui| {
                ui.label("來源唯讀；以結構化事件、物件條件與開局鏡頭編輯，輸出新工作副本。");
                ui.horizontal(|ui| {
                    ui.label("來源地圖目錄");
                    ui.text_edit_singleline(&mut self.load_path);
                    if ui.button("載入").clicked() {
                        self.request_load(PathBuf::from(self.load_path.trim()))
                    }
                });
                if let Some(s) = &self.source {
                    ui.small(format!("原始 BCI：{}", s.path.display()));
                    ui.small(if self.aliases.is_empty() { "未找到遊戲別名 catalog；別名需手動核對。".into() }
                    else { format!("已載入 {} 個遊戲 ObjDefName 別名，編譯時會驗證。", self.aliases.len()) });
                }
                if self.dirty() {
                    ui.colored_label(egui::Color32::YELLOW, "尚未輸出的修改");
                }
                egui::ScrollArea::vertical()
                    .max_height(650.)
                    .show(ui, |ui| {
                        ui.collapsing("開局鏡頭", |ui| {
                            ui.checkbox(&mut self.camera.enabled, "啟用");
                            ui.horizontal(|ui| {
                                for (name, v) in [
                                    ("X", &mut self.camera.x),
                                    ("Y", &mut self.camera.y),
                                    ("Z", &mut self.camera.z),
                                    ("Zoom", &mut self.camera.zoom),
                                ] {
                                    ui.label(name);
                                    ui.add(egui::DragValue::new(v).speed(0.25));
                                }
                            });
                            egui::ComboBox::from_id_source("camera_primitive")
                                .selected_text(if self.camera.primitive == 0 {
                                    "世界焦點 / centerToPos"
                                } else {
                                    "引擎座標 / enginePos"
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.camera.primitive, 0, "世界焦點");
                                    ui.selectable_value(&mut self.camera.primitive, 1, "引擎座標");
                                });
                        });
                        ui.collapsing("場景生成物件與持久 ID", |ui| {
                            let mut remove = None;
                            for (i, s) in self.document.spawns.iter_mut().enumerate() {
                                ui.push_id(i, |ui| {
                                    if s.prebuilt {
                                        ui.label(format!("預建 DATA 唯讀：{} / ID {} / X {} Z {} team {} angle {}", s.alias, s.id, s.x, s.z, s.team, s.angle));
                                        return;
                                    }
                                    ui.horizontal(|ui| {
                                        ui.text_edit_singleline(&mut s.alias);
                                        alias_combo(ui, &mut s.alias, &self.aliases);
                                        ui.label("team");
                                        ui.add(egui::DragValue::new(&mut s.team));
                                        ui.label("count (0=物件)");
                                        ui.add(egui::DragValue::new(&mut s.count));
                                        if ui.button("刪除").clicked() {
                                            remove = Some(i)
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        for (label, v) in [("X", &mut s.x), ("Z", &mut s.z)] {
                                            ui.label(label);
                                            ui.add(egui::DragValue::new(v));
                                        }
                                        ui.label("角度");
                                        ui.add(egui::DragValue::new(&mut s.angle));
                                        ui.label(if s.prebuilt {
                                            "既有 DATA 預建；需保留 binding"
                                        } else {
                                            "開局腳本生成"
                                        });
                                    });
                                    ui.small(format!("ID {}", s.id));
                                });
                            }
                            if let Some(i) = remove {
                                self.document.spawns.remove(i);
                            }
                            if ui.button("新增腳本生成物件").clicked() {
                                self.document.spawns.push(Spawn {
                                    id: scenario::new_spawn_id(),
                                    alias: "unit".into(),
                                    x: 8000.,
                                    z: 8000.,
                                    team: 0,
                                    count: 10,
                                    ..Default::default()
                                });
                            }
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("新增事件").clicked() {
                                self.document.events.push(Event {
                                    name: format!("事件 {}", self.document.events.len() + 1),
                                    actions: vec![Action {
                                        text: "Message".into(),
                                        ..Default::default()
                                    }],
                                    ..Default::default()
                                });
                                self.selected = self.document.events.len() - 1;
                            }
                            if !self.document.events.is_empty()
                                && ui.button("刪除選取事件").clicked()
                            {
                                self.document
                                    .events
                                    .remove(self.selected.min(self.document.events.len() - 1));
                                self.selected = self
                                    .selected
                                    .min(self.document.events.len().saturating_sub(1));
                            }
                        });
                        for (i, e) in self.document.events.iter().enumerate() {
                            if ui
                                .selectable_label(
                                    self.selected == i,
                                    format!("{} {}", if e.enabled { "●" } else { "○" }, e.name),
                                )
                                .clicked()
                            {
                                self.selected = i;
                            }
                        }
                        let targets: Vec<_> = self
                            .document
                            .spawns
                            .iter()
                            .map(|s| (s.clone(), format!("{} / {}", s.alias, s.id)))
                            .collect();
                        if let Some(e) = self.document.events.get_mut(self.selected) {
                            event_editor(ui, e, &targets, &self.aliases);
                        }
                    });
                ui.separator();
                if ui.button("驗證並編譯預覽").clicked() {
                    if let Err(e) = self.preview() {
                        self.error = Some(e)
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("新工作副本目錄");
                    ui.text_edit_singleline(&mut self.export_path);
                    if ui.button("輸出 JSON + BCI + 原版備份").clicked() {
                        let path = PathBuf::from(self.export_path.trim());
                        if let Err(e) = self.export_new(&path) {
                            self.error = Some(e)
                        }
                    }
                });
                ui.label(&self.report);
                if let Some(e) = &self.error {
                    ui.colored_label(egui::Color32::RED, e);
                }
            });
        if !opened {
            if self.dirty() {
                self.pending = Some(Pending::Close)
            } else {
                self.open = false
            }
        }
        if let Some(pending) = self.pending.clone() {
            egui::Window::new("尚未輸出的劇本修改")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label("載入或關閉會放棄目前修改。可取消後先輸出工作副本。");
                    ui.horizontal(|ui| {
                        if ui.button("放棄修改並繼續").clicked() {
                            self.pending = None;
                            match pending {
                                Pending::Close => self.open = false,
                                Pending::Load(p) => {
                                    if let Err(e) = self.load_now(&p) {
                                        self.error = Some(e)
                                    }
                                }
                            }
                        }
                        if ui.button("取消").clicked() {
                            self.pending = None
                        }
                    });
                });
        }
    }
}

fn event_editor(ui: &mut egui::Ui, e: &mut Event, targets: &[(Spawn, String)], aliases: &[String]) {
    ui.separator();
    ui.horizontal(|ui| {
        ui.checkbox(&mut e.enabled, "啟用事件");
        ui.text_edit_singleline(&mut e.name);
    });
    ui.horizontal(|ui| {
        ui.label("開局後延遲（秒）");
        ui.add(egui::DragValue::new(&mut e.delay_seconds).clamp_range(0..=86400));
        ui.checkbox(&mut e.repeat, "週期重複");
    });
    ui.label("條件：全部成立且延遲到期後執行；死亡條件先追蹤曾存在的物件。");
    let mut remove = None;
    for (i, c) in e.conditions.iter_mut().enumerate() {
        ui.push_id(("condition", i), |ui| {
            ui.horizontal(|ui| {
                kind_combo(
                    ui,
                    "kind",
                    &mut c.kind,
                    &["物件存在", "死亡或已移除", "位於矩形區域"],
                );
                if ui.button("刪除此條件").clicked() {
                    remove = Some(i)
                }
            });
            egui::ComboBox::from_id_source("target")
                .selected_text(
                    targets
                        .iter()
                        .find(|(s, _)| s.id == c.target_id)
                        .map(|(_, n)| n.as_str())
                        .unwrap_or("請選擇目標 / 已刪除"),
                )
                .show_ui(ui, |ui| {
                    for (s, name) in targets {
                        ui.selectable_value(&mut c.target_id, s.id, name);
                    }
                });
            if c.kind == 2 {
                ui.horizontal(|ui| {
                    for (name, v) in [
                        ("min X", &mut c.min_x),
                        ("min Z", &mut c.min_z),
                        ("max X", &mut c.max_x),
                        ("max Z", &mut c.max_z),
                    ] {
                        ui.label(name);
                        ui.add(egui::DragValue::new(v).clamp_range(0..=16383));
                    }
                });
            }
        });
    }
    if let Some(i) = remove {
        e.conditions.remove(i);
    }
    if ui.button("新增物件條件").clicked() {
        let mut c = Condition::default();
        if let Some((s, _)) = targets.first() {
            c.target_id = s.id
        }
        e.conditions.push(c);
    }
    ui.label("動作：依序執行。勝敗動作只能放在單次事件最後。");
    let mut remove = None;
    for (i, a) in e.actions.iter_mut().enumerate() {
        ui.push_id(("action", i), |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    kind_combo(
                        ui,
                        "kind",
                        &mut a.kind,
                        &["顯示訊息", "隊伍外交", "生成部隊", "勝利", "失敗"],
                    );
                    if ui.button("刪除此動作").clicked() {
                        remove = Some(i)
                    }
                });
                match a.kind {
                    0 => {
                        ui.text_edit_multiline(&mut a.text);
                        ui.small("文字需能使用 Windows-1251 編碼，最多 1000 bytes。");
                    }
                    1 => {
                        ui.horizontal(|ui| {
                            ui.label("team");
                            ui.add(egui::DragValue::new(&mut a.team).clamp_range(0..=7));
                            ui.label("other team");
                            ui.add(egui::DragValue::new(&mut a.other_team).clamp_range(0..=7));
                            ui.checkbox(&mut a.hostile, "敵對");
                        });
                    }
                    2 => {
                        ui.horizontal(|ui| {
                            ui.label("ObjDefName 別名");
                            ui.text_edit_singleline(&mut a.alias);
                            alias_combo(ui, &mut a.alias, aliases);
                            ui.label("team");
                            ui.add(egui::DragValue::new(&mut a.team).clamp_range(0..=7));
                            ui.label("count");
                            ui.add(egui::DragValue::new(&mut a.count).clamp_range(1..=20));
                        });
                        ui.horizontal(|ui| {
                            ui.label("X");
                            ui.add(egui::DragValue::new(&mut a.x).clamp_range(0. ..=16383.));
                            ui.label("Z");
                            ui.add(egui::DragValue::new(&mut a.z).clamp_range(0. ..=16383.));
                        });
                        let mut moved = a.move_destination.is_some();
                        if ui.checkbox(&mut moved, "生成成功後移動至目的地").changed() {
                            a.move_destination = if moved {
                                Some(WorldPosition { x: 8000, z: 8000 })
                            } else {
                                None
                            }
                        }
                        if let Some(p) = &mut a.move_destination {
                            ui.horizontal(|ui| {
                                ui.label("目的 X");
                                ui.add(egui::DragValue::new(&mut p.x).clamp_range(0..=16383));
                                ui.label("目的 Z");
                                ui.add(egui::DragValue::new(&mut p.z).clamp_range(0..=16383));
                            });
                        }
                    }
                    3 | 4 => {
                        ui.small("結束任務；後續事件停止輪詢。");
                    }
                    _ => {
                        ui.colored_label(egui::Color32::RED, "不支援的動作種類");
                    }
                }
                if a.kind != 2 {
                    a.move_destination = None
                }
            });
        });
    }
    if let Some(i) = remove {
        e.actions.remove(i);
    }
    if ui.button("新增動作").clicked() {
        e.actions.push(Action {
            text: "Message".into(),
            ..Default::default()
        });
    }
}
fn kind_combo(ui: &mut egui::Ui, id: &str, value: &mut i32, names: &[&str]) {
    egui::ComboBox::from_id_source(id)
        .selected_text(names.get(*value as usize).copied().unwrap_or("未知"))
        .show_ui(ui, |ui| {
            for (i, name) in names.iter().enumerate() {
                ui.selectable_value(value, i as i32, *name);
            }
        });
}
/// Creates only a new destination. Rollback removes exactly this call's created files,
/// preserving any pre-existing directories/files; no recursive deletion is used.
fn export_files(target: &Path, files: &[(&str, Vec<u8>)], fail_after: Option<usize>) -> Result<()> {
    let target = safe_parent(target)?;
    if target.exists() {
        return Err("output already exists".into());
    }
    fs::create_dir(&target).map_err(|e| e.to_string())?;
    let script = target.join("SCRIPT");
    let mut created = vec![];
    let result = (|| -> Result<()> {
        fs::create_dir(&script).map_err(|e| e.to_string())?;
        for (i, (name, bytes)) in files.iter().enumerate() {
            if fail_after == Some(i) {
                return Err("injected export failure".into());
            }
            if !matches!(
                *name,
                "arm_scenario.json"
                    | "arm_start_camera.json"
                    | "SCRIPT/ak_level.arm_original"
                    | "SCRIPT/ak_level.bci"
            ) {
                return Err("unexpected export filename".into());
            }
            safe_parent(&target)?;
            let path = target.join(name);
            safe_parent(&path)?;
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .map_err(|e| e.to_string())?;
            created.push(path);
            file.write_all(bytes).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut rollback_errors = vec![];
        for path in created.iter().rev() {
            if let Err(e) = fs::remove_file(path) {
                rollback_errors.push(e.to_string())
            }
        }
        if let Err(e) = fs::remove_dir(&script) {
            if script.exists() {
                rollback_errors.push(e.to_string())
            }
        }
        if let Err(e) = fs::remove_dir(&target) {
            rollback_errors.push(e.to_string())
        }
        if !rollback_errors.is_empty() {
            return Err(format!("{error}; rollback: {}", rollback_errors.join("; ")));
        }
        return Err(error);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    fn scratch() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "arm-scenario-ui-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir(&p).unwrap();
        p
    }
    fn source() -> PathBuf {
        let p = scratch();
        fs::create_dir(p.join("SCRIPT")).unwrap();
        fs::write(
            p.join("SCRIPT/ak_level.bci"),
            scenario::BciImage::idle().serialize(),
        )
        .unwrap();
        p
    }
    #[test]
    fn load_export_and_reload() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        assert!(!ui.dirty());
        ui.document.events.push(Event {
            name: "e".into(),
            actions: vec![Action {
                text: "hi".into(),
                ..Default::default()
            }],
            ..Default::default()
        });
        ui.camera.enabled = true;
        assert!(ui.dirty());
        let out = p.join("export");
        ui.export_new(&out).unwrap();
        assert!(!ui.dirty());
        assert_eq!(
            fs::read(out.join("SCRIPT/ak_level.arm_original")).unwrap(),
            scenario::BciImage::idle().serialize()
        );
        let bytes = fs::read(out.join("SCRIPT/ak_level.bci")).unwrap();
        let mut next = ScenarioUi::default();
        next.load_now(&out).unwrap();
        assert_eq!(next.document.events.len(), 1);
        assert!(next.camera.enabled);
        assert_eq!(next.preview().unwrap(), bytes);
        assert!(ui.export_new(&out).is_err());
        assert_eq!(fs::read(out.join("SCRIPT/ak_level.bci")).unwrap(), bytes);
    }
    #[test]
    fn failed_export_rolls_back_each_step() {
        let p = scratch();
        let files = vec![
            ("arm_scenario.json", vec![1]),
            ("arm_start_camera.json", vec![2]),
            ("SCRIPT/ak_level.arm_original", vec![3]),
            ("SCRIPT/ak_level.bci", vec![4]),
        ];
        for i in 0..4 {
            let out = p.join(format!("failure-{i}"));
            assert!(export_files(&out, &files, Some(i)).is_err());
            assert!(!out.exists());
            export_files(&out, &files, None).unwrap();
        }
    }
    #[test]
    fn preexisting_and_program_files_refused() {
        let p = scratch();
        fs::write(p.join("keep"), [1]).unwrap();
        assert!(export_files(&p, &[], None).is_err());
        assert_eq!(fs::read(p.join("keep")).unwrap(), [1]);
        let parent = p.join("Program Files");
        fs::create_dir(&parent).unwrap();
        assert!(safe_parent(&parent.join("new")).is_err());
        assert!(safe_parent(&p.join("../new")).is_err());
    }
    #[test]
    fn dirty_load_requires_explicit_discard() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        ui.camera.x = 100.;
        let target = source();
        ui.request_load(target.clone());
        assert!(matches!(ui.pending, Some(Pending::Load(_))));
        assert_eq!(ui.camera.x, 100.);
        ui.pending = None;
        assert!(ui.dirty());
        ui.load_now(&target).unwrap();
        assert!(!ui.dirty());
    }
    #[test]
    fn externally_changed_original_blocks_export() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        fs::write(p.join("SCRIPT/ak_level.bci"), b"changed").unwrap();
        let out = p.join("out");
        assert!(ui.export_new(&out).is_err());
        assert!(!out.exists());
    }
    #[test]
    fn invalid_edit_does_not_create_output() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        ui.camera.enabled = true;
        ui.camera.zoom = 20.;
        let out = p.join("out");
        assert!(ui.export_new(&out).is_err());
        assert!(!out.exists());
    }
    #[test]
    fn native_ui_layout_frame() {
        let ctx = egui::Context::default();
        let mut ui = ScenarioUi {
            open: true,
            ..Default::default()
        };
        ctx.begin_frame(Default::default());
        ui.show(&ctx);
        let output = ctx.end_frame();
        assert!(!output.shapes.is_empty());
    }
    #[cfg(windows)]
    #[test]
    fn junction_output_refused() {
        use std::process::Command;
        let p = scratch();
        let real = p.join("real");
        let link = p.join("link");
        fs::create_dir(&real).unwrap();
        let status = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(&real)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(safe_parent(&link.join("new")).is_err());
        assert!(!real.join("new").exists());
        fs::remove_dir(link).unwrap();
    }
}

fn alias_combo(ui: &mut egui::Ui, alias: &mut String, names: &[String]) {
    if names.is_empty() {
        return;
    }
    egui::ComboBox::from_id_source("object_alias_catalog")
        .selected_text("從遊戲別名選擇")
        .show_ui(ui, |ui| {
            for name in names {
                ui.selectable_value(alias, name.clone(), name);
            }
        });
}
