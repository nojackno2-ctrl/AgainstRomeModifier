//! Safe Save 提交後僅重試失敗的面板；所有比較皆以提交前固定的內容為準。
use super::*;
use against_rome_maps::{MapSaveTransaction, StagedAction};
use against_rome_scenario::ScenarioDocument;
use serde_json::Value;
use std::collections::BTreeMap;

type Contents = BTreeMap<PathBuf, Vec<u8>>;

/// 保留完整位元組，不使用可能漏掉同長度修改的 metadata 指紋。
fn read_tree(root: &Path) -> Result<Contents, String> {
    fn visit(root: &Path, dir: &Path, out: &mut Contents) -> Result<(), String> {
        for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let rel = path.strip_prefix(root).map_err(|e| e.to_string())?;
            against_rome_maps::verify_path_components_safe(root, rel)?;
            let ty = entry.file_type().map_err(|e| e.to_string())?;
            if ty.is_dir() {
                visit(root, &path, out)?;
            } else if ty.is_file() {
                out.insert(
                    rel.to_path_buf(),
                    std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?,
                );
            } else {
                return Err(format!(
                    "無法確認檔案內容 / Cannot verify file: {}",
                    path.display()
                ));
            }
        }
        Ok(())
    }
    let mut result = Contents::new();
    if root.try_exists().map_err(|e| e.to_string())? {
        visit(root, root, &mut result)?;
    }
    Ok(result)
}

fn identity(path: &Path) -> Result<PathBuf, String> {
    let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
    if absolute.try_exists().map_err(|e| e.to_string())? {
        absolute.canonicalize().map_err(|e| e.to_string())
    } else {
        let parent = absolute
            .parent()
            .ok_or("無法確認地圖路徑 / Invalid map path")?;
        Ok(identity(parent)?.join(absolute.file_name().ok_or("Invalid map name")?))
    }
}

fn optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Windows 也可能把「父路徑是一般檔案」回報為 NotFound，不能當成正常缺檔。
            for parent in path.ancestors().skip(1) {
                if parent.try_exists().map_err(|e| e.to_string())? {
                    if !parent.is_dir() {
                        return Err(format!(
                            "來源父路徑不是目錄 / Source parent is not a directory: {}",
                            parent.display()
                        ));
                    }
                    break;
                }
            }
            Ok(None)
        }
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

pub(super) enum PendingPanelSync {
    Emboss {
        heights: Vec<u8>,
        green: Vec<u8>,
    },
    Placement(crate::placement_ui::PlacementSyncState),
    Scenario {
        draft: (Value, crate::event_graph::model::EventGraph),
        campaign: crate::campaign_ui::CameraHistoryState,
        camera_pending: bool,
        final_document: Option<ScenarioDocument>,
    },
    Textures(Value),
    Scene {
        draft: crate::scene_ui::SceneSnapshot,
        token: u64,
        map: Option<PathBuf>,
    },
}
impl PendingPanelSync {
    pub(super) fn scenario(
        app: &EditorApp,
        camera_pending: bool,
        final_document: Option<ScenarioDocument>,
    ) -> Self {
        Self::Scenario {
            draft: app.scenario.sync_state(),
            campaign: camera_state(app),
            camera_pending,
            final_document,
        }
    }
    pub(super) fn scene(app: &EditorApp) -> Self {
        Self::Scene {
            draft: app.scene.snapshot(),
            token: app.scene.history_token(),
            map: app.scene.map_dir().map(Path::to_path_buf),
        }
    }
    fn name(&self) -> &'static str {
        match self {
            Self::Emboss { .. } => "emboss baseline",
            Self::Placement(_) => "placement baseline",
            Self::Scenario { .. } => "scenario baseline",
            Self::Textures(_) => "texture baseline",
            Self::Scene { .. } => "scene baseline",
        }
    }
    fn unchanged(&self, app: &EditorApp) -> bool {
        match self {
            Self::Emboss { heights, green } => app
                .session
                .as_ref()
                .zip(app.emboss.as_ref())
                .is_some_and(|(s, e)| {
                    s.heights() == heights
                        && e.session.build_emboss(s.heights()).ok().flatten().as_ref()
                            == Some(green)
                }),
            Self::Placement(draft) => &app.placement.sync_state() == draft,
            Self::Scenario {
                draft, campaign, ..
            } => &app.scenario.sync_state() == draft && &camera_state(app) == campaign,
            Self::Textures(draft) => &app.textures.sync_state() == draft,
            Self::Scene { draft, token, map } => {
                &app.scene.snapshot() == draft
                    && app.scene.history_token() == *token
                    && app.scene.map_dir() == map.as_deref()
            }
        }
    }
    fn synchronize(&self, app: &mut EditorApp, path: &Path) -> Result<(), String> {
        #[cfg(test)]
        if app.panel_sync_faults.contains(self.name()) {
            return Err("暫時無法同步（測試故障）".into());
        }
        match self {
            Self::Emboss { heights, green } => app
                .emboss
                .as_mut()
                .ok_or("Missing emboss")?
                .session
                .commit_baseline(heights, Some(green))
                .map_err(|e| e.to_string()),
            Self::Placement(_) => app.placement.commit_baseline(path),
            Self::Scenario {
                camera_pending,
                final_document,
                ..
            } => {
                if *camera_pending {
                    app.scenario.load_session(path)?;
                    app.campaign.camera_saved();
                    Ok(())
                } else {
                    app.scenario
                        .commit_baseline_with(path, final_document.as_ref())
                }
            }
            Self::Textures(_) => app.textures.commit_baseline(path),
            Self::Scene { .. } => app.scene.commit_baseline(path),
        }
    }
}
fn camera_state(app: &EditorApp) -> crate::campaign_ui::CameraHistoryState {
    app.campaign.camera_history_state()
}

pub(super) struct PostCommitSync {
    path: PathBuf,
    loaded_path: Option<PathBuf>,
    canonical: PathBuf,
    expected: Contents,
    // Scenario 載入也會讀地圖外的別名檔；缺檔亦是需比對的基準狀態。
    aliases: Option<(PathBuf, Option<Vec<u8>>)>,
    pub(super) failed: Vec<PendingPanelSync>,
}
impl PostCommitSync {
    pub(super) fn prepare(
        app: &EditorApp,
        path: &Path,
        tx: &MapSaveTransaction,
        failed: Vec<PendingPanelSync>,
    ) -> Result<Self, String> {
        let mut expected = read_tree(path)?;
        // 在 commit 前固定交易產物；絕不從提交後的磁碟重新建立真值。
        for entry in tx.staged_entries() {
            match entry.action() {
                StagedAction::WriteBytes(bytes) => {
                    expected.insert(entry.relative_path().to_path_buf(), bytes.clone());
                }
                StagedAction::CopyFromPath(source) => {
                    expected.insert(
                        entry.relative_path().to_path_buf(),
                        std::fs::read(source).map_err(|e| e.to_string())?,
                    );
                }
                StagedAction::Delete => {
                    expected.remove(entry.relative_path());
                }
            }
        }
        let aliases = path
            .parent()
            .and_then(Path::parent)
            .map(|root| {
                let path = root.join("SYSTEM/CLAK/cl_scint.ini");
                optional_bytes(&path).map(|bytes| (path, bytes))
            })
            .transpose()?;
        Ok(Self {
            path: path.to_path_buf(),
            loaded_path: app.loaded_map_path.clone(),
            canonical: identity(path)?,
            expected,
            aliases,
            failed,
        })
    }
    fn verify(&self, app: &EditorApp) -> Result<(), String> {
        if Path::new(app.map_path_input.trim()) != self.path
            || app.loaded_map_path != self.loaded_path
            || identity(&self.path)? != self.canonical
        {
            return Err("地圖路徑已切換，拒絕重試 / Map path changed".into());
        }
        if read_tree(&self.path)? != self.expected {
            return Err(
                "磁碟內容與已提交交易不符，拒絕重試 / Committed map contents changed".into(),
            );
        }
        if let Some((path, bytes)) = &self.aliases {
            if &optional_bytes(path)? != bytes {
                return Err("劇本別名來源已變更 / Scenario aliases changed".into());
            }
        }
        Ok(())
    }
    /// 成功項目立即移出，不會在下次重試再次重載；失敗項目保留原始草稿。
    pub(super) fn synchronize(&mut self, app: &mut EditorApp, retry: bool) -> Vec<String> {
        if let Err(e) = self.verify(app) {
            return vec![format!(
                "{}: {e}",
                self.failed
                    .iter()
                    .map(PendingPanelSync::name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )];
        }
        let mut errors = Vec::new();
        self.failed.retain(|panel| {
            let result = if retry && !panel.unchanged(app) {
                Err("提交後面板已有新編輯或切換來源，保留草稿並拒絕同步 / Panel changed since commit".into())
            } else { panel.synchronize(app, &self.path) };
            if let Err(e) = result { errors.push(format!("{}: {e}", panel.name())); true } else { false }
        });
        errors
    }
}
impl EditorApp {
    /// 僅在內容與原始交易一致時重試，不寫入地圖。
    pub fn retry_panel_sync(&mut self) {
        let Some(mut sync) = self.post_commit_sync.take() else {
            self.set_status(
                "無可安全重試的交易紀錄 / No verified transaction to retry".into(),
                true,
            );
            return;
        };
        let mut errors = sync.synchronize(self, true);
        if errors.is_empty() {
            // 指紋讀取失敗時仍保持阻擋，不能解除警告或將未知磁碟當成新來源。
            match crate::recovery::fingerprint(&sync.path) {
                Ok(source) => {
                    self.recovery.source = source;
                    self.post_commit_warning = None;
                    self.reset_journal_epoch();
                    self.recovery_deferred = false;
                    if let Err(e) = self.recovery.clear(&sync.path) {
                        self.set_status(
                            format!("面板已同步，舊復原檔清除失敗 / Recovery cleanup failed: {e}"),
                            true,
                        );
                        return;
                    }
                    if self.has_unsaved_changes() {
                        let pending = self.recovery_pending();
                        if let Err(e) = self.recovery.save(&sync.path, true, pending) {
                            self.set_status(
                                format!(
                                    "面板已同步，新增編輯快照失敗 / New edit snapshot failed: {e}"
                                ),
                                true,
                            );
                            return;
                        }
                    }
                    self.set_status(
                        tr(
                            self.lang(),
                            "Panel sync completed; pending edits were preserved.",
                            "面板同步完成，已保留其他待處理編輯。",
                        )
                        .into(),
                        false,
                    );
                    return;
                }
                Err(e) => errors.push(e),
            }
        }
        let warning = errors.join("; ");
        self.post_commit_warning = Some(warning.clone());
        self.post_commit_sync = Some(sync);
        self.set_status(
            format!(
                "{} {warning}",
                tr(
                    self.lang(),
                    "Panel sync failed; retry is available.",
                    "面板同步失敗，可再次重試。"
                )
            ),
            true,
        );
    }
    pub(super) fn panel_sync_retry_ui(&mut self, ui: &mut egui::Ui) {
        if self.post_commit_warning.is_some()
            && ui
                .button(tr(self.lang(), "Retry panel sync", "重試面板同步"))
                .clicked()
        {
            self.retry_panel_sync();
        }
    }
}

#[cfg(test)]
#[path = "app_post_commit_sync_tests.rs"]
mod tests;
