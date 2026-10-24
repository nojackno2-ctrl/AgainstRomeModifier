//! 編輯器生命週期與當機復原整合。
use super::*;
use crate::recovery::AUTOSAVE_INTERVAL;
use serde_json::{json, Value};
impl EditorApp {
    pub(super) fn recovery_pending(&self) -> Value {
        let omitted: Vec<&str> = [
            (self.spawn_ui.pending_starter_kit().is_some(), "Scenario with pending starter kit DATA families / 含待套用起始配置 DATA 家族的劇本"),
            (self.session.as_ref().zip(self.emboss.as_ref()).is_some_and(|(s, e)| e.session.is_dirty(s.heights())), "Emboss / 地形浮雕圖層"),
            (self.placement.is_dirty(), "DATA placement / DATA 放置"),
            (self.nature.session.is_dirty(), "Nature / 自然物件"),
            (self.scene.is_dirty(), "Scene / 場景"),
            (
                self.scenario.graph_ui().is_dirty() || self.event_graph.is_dirty(),
                "Unapplied event graph / 未套用事件圖",
            ),
            (self.maps.is_dirty(), "Map metadata / 地圖中繼資料"),
            (self.environment.is_dirty(), "Environment / 環境"),
            (self.landscape.is_dirty(), "Landscape / 景觀"),
            (self.packaging.is_dirty(), "Packaging / 打包"),
            (
                self.campaign.has_unsupported_edits(),
                "Campaign objectives/waves/cinematics / 戰役目標、波次與運鏡",
            ),
            (self.ai_planner.is_dirty(), "AI planner / AI 規劃"),
            (self.ambience.is_dirty(), "Ambience / 音景"),
            (self.toolbox.is_dirty(), "Toolbox / 工具箱"),
        ]
        .into_iter()
        .filter_map(|(dirty, name)| dirty.then_some(name))
        .collect();
        let terrain = self.session.as_ref().map(|s| json!({"vertex_size":s.vertex_size,"collision_size":s.collision_size,"heights":s.heights(),"passability":s.collision()}));
        json!({"terrain":terrain,"materials":self.textures.session_textures(),
            "water":[self.water_surface,self.water_bed],"reset_auxiliary_layers":self.reset_auxiliary_layers,
            "scenario":(self.scenario.is_loaded() && self.spawn_ui.pending_starter_kit().is_none()).then(|| self.scenario.recovery_state()),
            "campaign_camera":self.campaign.start_camera,"camera_pending":self.campaign.camera_pending(),"camera_dirty":self.campaign.is_dirty(),
            "omitted_panels":omitted})
    }
    pub(super) fn clear_recovery(&mut self) {
        if let Some(map) = self.loaded_map_path.clone() {
            if let Err(e) = self.recovery.clear(&map) {
                self.set_status(
                    format!("Recovery cleanup failed / 復原檔清除失敗：{e}"),
                    true,
                );
            }
        }
    }
    pub(super) fn recovery_tick(&mut self, ctx: &egui::Context) {
        if let Some(e) = self.recovery.poll() {
            self.set_status(format!("Autosave failed / 自動快照失敗：{e}"), true);
        }
        ctx.request_repaint_after(AUTOSAVE_INTERVAL);
        // 僅清除本輪已寫入或已套用的快照，保留尚待選擇的舊快照。
        if !self.recovery_deferred && self.recovery.offer.is_none()
            && self.recovery.last.is_some() && !self.has_unsaved_changes() {
            self.clear_recovery();
        }
        if self.recovery.checked.elapsed() < AUTOSAVE_INTERVAL || self.recovery.offer.is_some() {
            return;
        }
        self.recovery.checked = std::time::Instant::now();
        if let Some(map) = self.loaded_map_path.clone() {
            if self.has_unsaved_changes() {
                let pending = self.recovery_pending();
                if let Err(e) = self.recovery.save(&map, true, pending) {
                    self.set_status(format!("Autosave failed / 自動快照失敗：{e}"), true);
                }
            }
        }
    }
    pub(super) fn restore_recovery(&mut self, value: &Value) -> Result<(), String> {
        let map = self.loaded_map_path.clone().ok_or("No loaded map")?;
        if self.has_unsaved_changes() {
            return Err("Unsaved edits exist; save or discard before restoring / 已有未儲存編輯，請先儲存或捨棄再復原".into());
        }
        if crate::recovery::fingerprint(&map)? != self.recovery.source
            || value["source_fingerprint"] != self.recovery.source
        {
            return Err("Map source changed / 地圖來源已變更".into());
        }
        let p = &value["pending"];
        crate::recovery::validate_pending(p)?;
        let t = &p["terrain"];
        let heights: Vec<u8> =
            serde_json::from_value(t["heights"].clone()).map_err(|e| e.to_string())?;
        let collision: Vec<u8> =
            serde_json::from_value(t["passability"].clone()).map_err(|e| e.to_string())?;
        let s = self.session.as_ref().ok_or("Missing terrain")?;
        if t["vertex_size"].as_u64() != Some(s.vertex_size as u64)
            || t["collision_size"].as_u64() != Some(s.collision_size as u64)
            || heights.len() != s.heights().len()
            || collision.len() != s.collision().len()
        {
            return Err("Terrain dimensions mismatch / 地形尺寸不符".into());
        }
        let (surface, bed): (f32, u8) =
            serde_json::from_value(p["water"].clone()).map_err(|e| e.to_string())?;
        if !surface.is_finite() {
            return Err("Invalid water / 水面資料無效".into());
        }
        let reset = p["reset_auxiliary_layers"]
            .as_bool()
            .ok_or("Invalid auxiliary flag")?;
        let camera: against_rome_campaign::cinematics::MapStartCamera =
            serde_json::from_value(p["campaign_camera"].clone()).map_err(|e| e.to_string())?;
        camera.validate()?;
        let camera_pending = p["camera_pending"].as_bool().ok_or("Invalid camera flag")?;
        let camera_dirty = p["camera_dirty"]
            .as_bool()
            .ok_or("Invalid camera dirty flag")?;
        // 先在獨立 session 驗證，損毀快照不會部分套用到目前工作。
        let mut scenario = ScenarioUi::default();
        if !p["scenario"].is_null() {
            scenario.load_session(&map)?;
            scenario.restore_recovery(p["scenario"].clone())?;
        }
        let mut textures = TexturesUi::default();
        if !p["materials"].is_null() {
            let materials: Vec<String> =
                serde_json::from_value(p["materials"].clone()).map_err(|e| e.to_string())?;
            textures.load_session(&map)?;
            textures.restore_recovery(&materials)?;
        }
        let mut campaign = CampaignUi::default();
        campaign.load_map_camera(&map)?;
        campaign.capture_camera(camera);
        if camera_pending {
            let files = if scenario.dirty() {
                scenario.stage_transaction_files(&map)?
            } else {
                vec![]
            };
            campaign.apply_camera_candidate(&map, &files, scenario.known_aliases())?;
        }
        if crate::recovery::fingerprint(&map)? != self.recovery.source {
            return Err("Map source changed / 地圖來源已變更".into());
        }
        self.session
            .as_mut()
            .ok_or("Missing terrain")?
            .restore_recovery(heights, collision)?;
        if !p["scenario"].is_null() {
            self.scenario = scenario;
        }
        if !p["materials"].is_null() {
            self.textures = textures;
        }
        if camera_dirty {
            self.campaign = campaign;
        }
        self.water_surface = surface;
        self.water_bed = bed;
        self.reset_auxiliary_layers = reset;
        self.texture_dirty = true;
        self.navigation.invalidate();
        self.toolbar_graph_source = None;
        self.scenario_autoload_tried = None;
        self.material_autoload_tried = None;
        self.recovery.last = Some(p.clone());
        self.set_status("Recovered unsaved work / 已復原未儲存工作".into(), false);
        Ok(())
    }
    pub(super) fn recovery_dialog(&mut self, ctx: &egui::Context) {
        if self.recovery_deferred {
            egui::Window::new("Recovery available / 有待處理復原檔")
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Existing snapshot retained; autosave paused until review. / 保留既有快照；檢視前暫停自動快照。");
                    if ui.button("Review recovery / 檢視復原檔").clicked() {
                        self.recovery_deferred = false;
                        if let Some(map) = self.loaded_map_path.clone() {
                            self.recovery.review(&map);
                        }
                    }
                });
        }
        let Some(offer) = self.recovery.offer.as_ref() else {
            return;
        };
        let mut choice = 0;
        egui::Window::new("Crash recovery / 當機復原").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            ui.label("Unsaved work snapshot found / 找到未儲存工作快照");
            match offer {
                Ok(value) => {
                    let time = crate::recovery::utc_timestamp(value["timestamp"].as_u64().unwrap_or_default());
                    ui.label(format!("Unsaved work from {time} was found / 找到 {time} 未儲存工作"));
                    ui.label("Restores memory only; Safe Save persists changes. / 僅復原記憶體；請以 Safe Save 儲存。");
                    ui.label("Undo: terrain, materials and scenario each have one step. Campaign camera and water controls have no undo. / 地形、材質與劇本各自一步復原；戰役鏡頭與水面控制值沒有 undo。");
                    ui.label(format!("Not recovered / 無法復原：{}", value["pending"]["omitted_panels"]));
                    ui.label("Listed omissions are not restored, including starter kit DATA families and emboss layers. / 列出的缺漏不會復原，包含起始配置 DATA 家族與浮雕圖層。");
                }
                Err(e) => { ui.colored_label(Color32::RED, e); }
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(offer.is_ok(), egui::Button::new("Restore / 復原")).clicked() { choice = 1; }
                if ui.button("Discard / 捨棄").clicked() { choice = 2; }
                if ui.button("Decide later / 稍後決定").clicked() { choice = 3; }
            });
        });
        match choice {
            1 => {
                if let Some(Ok(value)) = self.recovery.offer.take() {
                    match self.restore_recovery(&value) {
                        Ok(()) => self.recovery.offer = None,
                        Err(e) => self.recovery.offer = Some(Err(e)),
                    }
                }
            }
            2 => self.clear_recovery(),
            3 => {
                self.recovery.offer = None;
                self.recovery_deferred = true;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recovery::{fingerprint, manifest, snapshot_path};
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn app() -> (EditorApp, Temp) {
        let root =
            Temp(std::env::temp_dir().join(format!("arm-recovery-app-{}", uuid::Uuid::new_v4())));
        let map = root.0.join("map");
        std::fs::create_dir_all(map.join("SCRIPT")).unwrap();
        crate::storage::safe_save_map(&map, &mut TerrainSession::create_blank(5, 32)).unwrap();
        std::fs::write(
            map.join("SCRIPT/ak_level.bci"),
            against_rome_scenario::BciImage::idle().serialize(),
        )
        .unwrap();
        let mut app = EditorApp::initial_state();
        app.recovery.root = Some(root.0.join("editor"));
        app.set_loaded_map(map, crate::storage::load_map(&root.0.join("map")).unwrap())
            .unwrap();
        (app, root)
    }
    fn edit(app: &mut EditorApp) {
        let s = app.session.as_mut().unwrap();
        s.apply_height_changes(&[crate::terrain::SampleChange {
            index: 0,
            before: 32,
            after: 70,
        }]);
        s.commit_stroke();
    }
    #[test]
    fn terrain_scenario_camera_spawns_roundtrip_no_map_writes_and_one_undo() {
        let (mut a, _root) = app();
        let map = a.loaded_map_path.clone().unwrap();
        a.scenario.load_session(&map).unwrap();
        std::fs::write(
            map.join("boden.txt"),
            b"[Dimension]\r\n2\r\n[Texturen]\r\n4BA___51\r\n4BA___51\r\n4BA___51\r\n4BA___51\r\n",
        )
        .unwrap();
        a.recovery.source = fingerprint(&map).unwrap();
        a.textures.load_session(&map).unwrap();
        a.textures
            .restore_recovery(&vec!["4BB___51".to_string(); 4])
            .unwrap();
        edit(&mut a);
        a.session
            .as_mut()
            .unwrap()
            .paint_collision(0.5, 0.5, 0.6, CollisionBrushOp::Block)
            .unwrap();
        a.session.as_mut().unwrap().commit_stroke();
        let mut state = a.scenario.recovery_state();
        state[0]["Events"] = json!([against_rome_scenario::Event {
            name: "復原事件".into(),
            actions: vec![against_rome_scenario::Action {
                text: "recovered".into(),
                ..Default::default()
            }],
            ..Default::default()
        }]);
        a.scenario.apply_spawns(vec![against_rome_scenario::Spawn {
            alias: "wolf".into(),
            id: uuid::Uuid::new_v4(),
            count: 1,
            x: 100.,
            z: 200.,
            ..Default::default()
        }]).unwrap();
        state[0] = serde_json::to_value(a.scenario.document()).unwrap();
        state[0]["Events"] = json!([against_rome_scenario::Event {
            name: "復原事件".into(),
            actions: vec![against_rome_scenario::Action { text: "recovered".into(), ..Default::default() }],
            ..Default::default()
        }]);
        state[1]["x"] = json!(1234.0);
        state[1]["enabled"] = json!(true);
        a.scenario.restore_recovery(state).unwrap();
        a.water_surface = 14.5;
        a.water_bed = 5;
        let pending = a.recovery_pending();
        let snapshot = manifest(&map, &a.recovery.source, pending.clone());
        let root = a.recovery.root.clone().unwrap();
        crate::recovery::write(&snapshot_path(&root, &map), &snapshot).unwrap();
        let snapshot = crate::recovery::read(&root, &map, &a.recovery.source)
            .unwrap()
            .unwrap();
        let before = fingerprint(&map).unwrap();
        let original = std::fs::read(map.join("boden.bmp")).unwrap();
        a.install_loaded_map(map.clone(), crate::storage::load_map(&map).unwrap())
            .unwrap();
        a.restore_recovery(&snapshot).unwrap();
        assert_eq!(a.scenario.document().spawns.len(), 1);
        assert_eq!(a.scenario.document().spawns[0].alias, "wolf");
        assert_eq!(a.scenario.recovery_state()[1]["x"], json!(1234.0));
        assert_eq!(a.scenario.recovery_state()[1]["enabled"], json!(true));
        assert_eq!(a.textures.session_textures().unwrap(), vec!["4BB___51".to_string(); 4]);
        assert_eq!(a.recovery_pending(), pending);
        assert!(a.has_unsaved_changes());
        assert_eq!(fingerprint(&map).unwrap(), before);
        assert_eq!(std::fs::read(map.join("boden.bmp")).unwrap(), original);
        let s = a.session.as_mut().unwrap();
        assert!(s.undo());
        assert_eq!(s.heights()[0], 32);
        assert_eq!(s.collision()[0], 0);
        assert!(!s.can_undo());
        assert!(!s.is_dirty());
        assert!(s.redo());
        assert_eq!(s.heights()[0], 70);
        assert!(a.textures.material_undo());
        assert_eq!(
            a.textures.session_textures().unwrap(),
            vec!["4BA___51".to_string(); 4]
        );
        assert!(!a.textures.can_material_undo());
        assert!(a.scenario.undo_events());
        assert!(a.scenario.document().events.is_empty());
        assert!(!a.scenario.can_undo_events());
        assert!(a.scenario.redo_events());
    }
    #[test]
    fn successful_safe_save_clears_snapshot() {
        let (mut a, _root) = app();
        let map = a.loaded_map_path.clone().unwrap();
        edit(&mut a);
        let pending = a.recovery_pending();
        a.recovery.save(&map, true, pending).unwrap();
        a.recovery.flush();
        let path = snapshot_path(a.recovery.root.as_ref().unwrap(), &map);
        assert!(path.exists());
        assert!(a
            .save_transaction(|_| true, |_| Err("模擬交易失敗".into()))
            .is_err());
        a.recovery.flush();
        assert!(path.exists());
        assert!(a.has_unsaved_changes());
        a.safe_save_current();
        assert!(!a.status_is_error, "{}", a.status_message);
        a.recovery.flush();
        assert!(!path.exists());
        assert!(!a.has_unsaved_changes());
    }
    #[test]
    fn recovery_bound_spawn_safe_save_moves_and_deletes_data() {
        use super::super::tests::spawn_save_tests as fixture;
        for delete in [false, true] {
            let (mut a, root, map) = fixture::fixture("recovery_bound", &[]);
            let _temp = Temp(root.clone());
            a.recovery.root = Some(root.join("editor"));
            fixture::place_house(&mut a, (30, 30), 0);
            fixture::save_ok(&mut a);
            fixture::sync(&mut a, &map);
            a.spawn_ui.select_all();
            if delete {
                a.spawn_ui.delete_selected(&mut a.scenario, &fixture::cx());
            } else {
                a.spawn_ui.move_selected(&mut a.scenario, &fixture::cx(), 512.0, 0.0, false);
            }
            assert!(!a.spawn_ui.status().1, "{}", a.spawn_ui.status().0);
            let pending = a.recovery_pending();
            let snapshot = manifest(&map, &a.recovery.source, pending);
            a.install_loaded_map(map.clone(), crate::storage::load_map(&map).unwrap()).unwrap();
            a.spawn_ui = crate::spawn_placement_ui::SpawnPlacementUi::default();
            a.restore_recovery(&snapshot).unwrap();
            fixture::sync(&mut a, &map);
            assert!(!a.spawn_ui.session().is_dirty());
            fixture::save_ok(&mut a);
            let objects = fixture::data_objects(&map);
            if delete { assert!(objects.is_empty()); }
            else {
                assert_eq!(objects.len(), 1);
                assert_eq!(objects[0].position[0], 8320.0);
                assert_eq!(objects[0].position[2], 7808.0);
            }
            assert!(!a.scenario.dirty());
        }
    }
    #[test]
    fn recovery_clean_undo_clears_current_snapshot_but_preserves_deferred_offer() {
        let (mut a, _root) = app();
        let map = a.loaded_map_path.clone().unwrap();
        let ctx = egui::Context::default();
        edit(&mut a);
        a.recovery.checked -= AUTOSAVE_INTERVAL;
        a.recovery_tick(&ctx);
        a.recovery.flush();
        let path = snapshot_path(a.recovery.root.as_ref().unwrap(), &map);
        assert!(path.exists());
        assert!(a.session.as_mut().unwrap().undo());
        a.recovery_tick(&ctx);
        a.recovery.flush();
        assert!(!path.exists());
        let p = a.recovery_pending();
        crate::recovery::write(&path, &manifest(&map, &a.recovery.source, p)).unwrap();
        let root = a.recovery.root.clone();
        a.recovery = crate::recovery::Recovery::default();
        a.recovery.root = root;
        a.recovery.open(&map);
        assert!(matches!(a.recovery.offer, Some(Ok(_))));
        a.recovery_tick(&ctx);
        a.recovery.flush();
        assert!(path.exists());
        a.recovery.offer = None;
        a.recovery_deferred = true;
        a.recovery.checked -= AUTOSAVE_INTERVAL;
        a.recovery_tick(&ctx);
        a.recovery.flush();
        assert!(path.exists());
    }
    #[test]
    fn recovery_gfxtype_change_refuses_restore() {
        let (mut a, _root) = app();
        let map = a.loaded_map_path.clone().unwrap();
        std::fs::create_dir_all(map.join("DATA")).unwrap();
        std::fs::write(map.join("DATA/gfxtype.dat"), b"original").unwrap();
        a.recovery.source = fingerprint(&map).unwrap();
        let pending = a.recovery_pending();
        let snapshot = manifest(&map, &a.recovery.source, pending.clone());
        std::fs::write(map.join("DATA/gfxtype.dat"), b"changed pool").unwrap();
        assert!(a.restore_recovery(&snapshot).unwrap_err().contains("source changed"));
        assert_eq!(a.recovery_pending(), pending);
    }
    #[test]
    fn recovery_reports_unserialized_emboss_relight() {
        let (mut a, _root) = app();
        let map = a.loaded_map_path.clone().unwrap();
        let raw = crate::bmp::BmpImage::parse(&std::fs::read(map.join("boden.bmp")).unwrap()).unwrap();
        let mut session = against_rome_editor::emboss::EmbossSession::new(5, vec![32; 25], Some(vec![128; 25])).unwrap();
        session.set_flat_relight_base(Some(80));
        a.emboss = Some(EmbossBaseline { raw, session });
        assert!(a.has_unsaved_changes());
        let p = a.recovery_pending();
        assert!(p["omitted_panels"].as_array().unwrap().iter().any(|v| v.as_str().unwrap().contains("Emboss")));
        let snapshot = manifest(&map, &a.recovery.source, p);
        assert!(!snapshot["included_parts"].as_array().unwrap().contains(&json!("emboss")));
    }
    #[test]
    fn timer_requires_loaded_dirty_changed_state_and_explicit_discard_cleans() {
        let (mut a, _root) = app();
        let map = a.loaded_map_path.clone().unwrap();
        let ctx = egui::Context::default();
        a.recovery.checked = std::time::Instant::now() - AUTOSAVE_INTERVAL;
        a.recovery_tick(&ctx);
        assert!(a.recovery.last.is_none());
        edit(&mut a);
        a.recovery_tick(&ctx);
        assert!(a.recovery.last.is_none());
        a.recovery.checked -= AUTOSAVE_INTERVAL;
        a.recovery_tick(&ctx);
        a.recovery.flush();
        let path = snapshot_path(a.recovery.root.as_ref().unwrap(), &map);
        assert!(path.exists());
        let bytes = std::fs::read(&path).unwrap();
        a.recovery.checked -= AUTOSAVE_INTERVAL;
        a.recovery_tick(&ctx);
        a.recovery.flush();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        a.clear_recovery();
        a.recovery.flush();
        assert!(!path.exists());
    }
    #[test]
    fn malformed_payload_and_changed_source_refuse_without_partial_restore() {
        let (mut a, _root) = app();
        let map = a.loaded_map_path.clone().unwrap();
        let p = a.recovery_pending();
        let mut snapshot = manifest(&map, &a.recovery.source, p.clone());
        snapshot["pending"]["water"] = json!("bad");
        assert!(a.restore_recovery(&snapshot).is_err());
        assert_eq!(a.recovery_pending(), p);
        let snapshot = manifest(&map, &a.recovery.source, p.clone());
        std::fs::write(map.join("boden.ini"), b"changed").unwrap();
        assert!(a.restore_recovery(&snapshot).is_err());
        assert_eq!(a.recovery_pending(), p);
    }
    #[test]
    fn recovered_terrain_discard_returns_to_loaded_baseline_and_clears_snapshot() {
        let (mut a, _root) = app();
        let map = a.loaded_map_path.clone().unwrap();
        edit(&mut a);
        let pending = a.recovery_pending();
        let snapshot = manifest(&map, &a.recovery.source, pending.clone());
        a.recovery.save(&map, true, pending).unwrap();
        a.recovery.flush();
        let path = snapshot_path(a.recovery.root.as_ref().unwrap(), &map);
        a.install_loaded_map(map.clone(), crate::storage::load_map(&map).unwrap())
            .unwrap();
        a.restore_recovery(&snapshot).unwrap();
        assert!(a.reset_terrain_now());
        a.recovery.flush();
        assert_eq!(a.session.as_ref().unwrap().heights()[0], 32);
        assert!(!a.has_unsaved_changes());
        assert!(!path.exists());
    }
    #[test]
    fn clean_close_removes_snapshot_and_deferred_choice_preserves_it() {
        let (mut a, _root) = app();
        let map = a.loaded_map_path.clone().unwrap();
        let p = a.recovery_pending();
        a.recovery.save(&map, true, p).unwrap();
        a.recovery.flush();
        let path = snapshot_path(a.recovery.root.as_ref().unwrap(), &map);
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        a.recovery_deferred = true;
        let _ = ctx.run(input.clone(), |ctx| a.ui_frame(ctx));
        a.recovery.flush();
        assert!(path.exists());
        a.recovery_deferred = false;
        let _ = ctx.run(input, |ctx| a.ui_frame(ctx));
        a.recovery.flush();
        assert!(!path.exists());
    }
    #[test]
    fn review_preserves_load_fingerprint_and_blank_detaches_offer() {
        let (mut a, _root) = app();
        let map = a.loaded_map_path.clone().unwrap();
        let original = a.recovery.source.clone();
        std::fs::write(map.join("boden.ini"), b"changed").unwrap();
        a.recovery.review(&map);
        assert!(matches!(a.recovery.offer, Some(Err(_))));
        assert_eq!(a.recovery.source, original);
        a.install_blank_map(5);
        assert!(a.recovery.offer.is_none());
        assert!(a.recovery.source.is_null());
    }
    #[test]
    fn autosave_write_failure_only_sets_status_without_dialog_or_map_writes() {
        let (mut a, root) = app();
        let map = a.loaded_map_path.clone().unwrap();
        edit(&mut a);
        let before = fingerprint(&map).unwrap();
        let blocked = root.0.join("blocked");
        std::fs::write(&blocked, b"file").unwrap();
        a.recovery.root = Some(blocked);
        a.recovery.checked -= AUTOSAVE_INTERVAL;
        let ctx = egui::Context::default();
        a.recovery_tick(&ctx);
        a.recovery.flush();
        a.recovery_tick(&ctx);
        assert!(a.status_is_error);
        assert!(a.status_message.contains("Autosave failed"));
        assert!(a.recovery.offer.is_none());
        assert!(a.has_unsaved_changes());
        assert_eq!(fingerprint(&map).unwrap(), before);
    }
}
