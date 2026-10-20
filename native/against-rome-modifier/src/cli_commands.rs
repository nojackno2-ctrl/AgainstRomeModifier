//! `backup`, `stats`, `profile`, `saves` and `help` commands plus text rendering, ported from
//! src.Modifier/Cli/CliCommands.cs and CliRunner.cs. JSON result keys are camelCase like the C# envelope.
use crate::{
    cli::{profile_to_json, CliOptions},
    save_manager::SaveBackupService,
    services::{BackupManager, PatchEngine},
    unit_stats, FeatureRegistry, PatchProfile, TroopConfig,
};
use against_rome_ambience::app_localization::{Language, Loc};
use serde_json::{json, Map, Value};
use std::{fmt::Write as _, fs, io, path::Path};

/// C# ArgumentException analog: surfaces as exit code 2.
pub fn arg_error(s: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, s.into())
}

/// Integral doubles print without a fraction, like System.Text.Json.
pub fn num(v: f64) -> Value {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 9.0e15 {
        Value::from(v as i64)
    } else {
        json!(v)
    }
}

fn utf8_bom(text: &str) -> Vec<u8> {
    let mut b = vec![0xEF, 0xBB, 0xBF];
    b.extend_from_slice(text.as_bytes());
    b
}

/// Indented JSON with caller-controlled key order (PascalCase DTO files must keep declaration order).
pub enum Ordered {
    Obj(Vec<(String, Ordered)>),
    Arr(Vec<Ordered>),
    Val(Value),
}
impl Ordered {
    pub fn render(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, 0);
        s
    }
    fn write(&self, out: &mut String, depth: usize) {
        let pad = |n: usize| "  ".repeat(n);
        match self {
            Ordered::Val(v) => out.push_str(&serde_json::to_string(v).unwrap()),
            Ordered::Arr(items) if items.is_empty() => out.push_str("[]"),
            Ordered::Obj(items) if items.is_empty() => out.push_str("{}"),
            Ordered::Arr(items) => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    out.push_str(&pad(depth + 1));
                    item.write(out, depth + 1);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad(depth));
                out.push(']');
            }
            Ordered::Obj(items) => {
                out.push_str("{\n");
                for (i, (k, item)) in items.iter().enumerate() {
                    let _ = write!(
                        out,
                        "{}{}: ",
                        pad(depth + 1),
                        serde_json::to_string(k).unwrap()
                    );
                    item.write(out, depth + 1);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad(depth));
                out.push('}');
            }
        }
    }
}

pub fn loc_for(o: &CliOptions) -> Loc {
    match o.language.as_deref().filter(|l| !l.trim().is_empty()) {
        Some(l) if l.eq_ignore_ascii_case("en") => Loc::new(Language::English),
        _ => Loc::new(Language::TraditionalChinese),
    }
}

fn game_with_backup(
    o: &CliOptions,
    resolve: impl Fn(&CliOptions) -> io::Result<std::path::PathBuf>,
) -> io::Result<(std::path::PathBuf, BackupManager)> {
    let game = resolve(o)?;
    let mut b = BackupManager::new();
    let base = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    b.load_backup_zip_to_memory(&game, None, &base)?;
    Ok((game, b))
}

pub fn backup_command(
    o: &CliOptions,
    resolve: impl Fn(&CliOptions) -> io::Result<std::path::PathBuf>,
) -> io::Result<Value> {
    let (game, mut b) = game_with_backup(o, resolve)?;
    let action = o
        .sub_action
        .clone()
        .unwrap_or_else(|| "status".into())
        .to_lowercase();
    let mut success = true;
    match action.as_str() {
        "ensure" => success = b.ensure_backup_loaded_for_game_path(&game)?,
        "heal" => {
            b.try_auto_heal_backup_files(&game);
        }
        _ => {}
    }
    let missing = b.find_missing_backup_resources();
    Ok(
        json!({"action": action, "gamePath": game.display().to_string(), "success": success, "missingCount": missing.len(), "missingFiles": missing}),
    )
}

fn unit_item(loc: &Loc, key: &str, stats: &[f64; 9]) -> Value {
    let m = TroopConfig::get_metadata(key);
    json!({"key": key, "name": loc.get_unit_name(key), "faction": m.map(|m| m.faction).unwrap_or(""), "tier": m.map(|m| m.tier).unwrap_or(""),
        "unitType": m.map(|m| m.unit_type).unwrap_or(""), "style": m.map(|m| m.style).unwrap_or(""),
        "hp": num(stats[0]), "damage": num(stats[1]), "defenseVw": num(stats[2]), "combatAw": num(stats[3]), "speed": num(stats[4]),
        "sight": num(stats[5]), "reloadRelt": num(stats[6]), "range": num(stats[7]), "spellRadius": num(stats[8])})
}

pub fn stats_command(
    o: &CliOptions,
    resolve: impl Fn(&CliOptions) -> io::Result<std::path::PathBuf>,
    loc: &Loc,
    now: &str,
) -> io::Result<Value> {
    let (game, b) = game_with_backup(o, resolve)?;
    let mut profile = PatchEngine::detect_current_patch_state(&game, &b);
    if let Some(v) = o.balance {
        profile.set_bool("Balance", v);
    }
    let action = o
        .sub_action
        .clone()
        .unwrap_or_else(|| "list".into())
        .to_lowercase();
    if action == "get" {
        let unit = o.sub_arg.clone().ok_or_else(|| {
            arg_error("請指定欲查詢的兵種代號 (例如: stats get FigGerInf01_Schwert)。")
        })?;
        let entries = TroopConfig::UNIT_METADATA_ENTRIES;
        let key = entries
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&unit))
            .or_else(|| {
                entries
                    .iter()
                    .find(|(k, _)| k.to_lowercase().contains(&unit.to_lowercase()))
            })
            .map(|(k, _)| *k)
            .ok_or_else(|| {
                arg_error(format!(
                    "未知的兵種代號: '{unit}'。可使用 'stats list' 查看所有可用代號。"
                ))
            })?;
        return Ok(unit_item(
            loc,
            key,
            &unit_stats::project(&b, key, &profile)?,
        ));
    }
    let mut items = Vec::new();
    let mut stats = Vec::new();
    for key in TroopConfig::sorted_units() {
        let s = unit_stats::project(&b, key, &profile)?;
        items.push(unit_item(loc, key, &s));
        stats.push((key.to_string(), s));
    }
    if action == "export" {
        let out = o
            .output
            .clone()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| arg_error("匯出兵種數值必須指定 --output <檔案路徑>。"))?;
        let format = o
            .format
            .clone()
            .unwrap_or_else(|| "ini".into())
            .to_lowercase();
        let text = if format == "json" {
            Ordered::Arr(
                items
                    .iter()
                    .map(|it| {
                        Ordered::Obj(
                            [
                                "key",
                                "name",
                                "faction",
                                "tier",
                                "unitType",
                                "style",
                                "hp",
                                "damage",
                                "defenseVw",
                                "combatAw",
                                "speed",
                                "sight",
                                "reloadRelt",
                                "range",
                                "spellRadius",
                            ]
                            .iter()
                            .map(|k| (pascal(k), Ordered::Val(it[*k].clone())))
                            .collect(),
                        )
                    })
                    .collect(),
            )
            .render()
        } else {
            unit_stats::write_preset(&stats, now)
        };
        fs::write(&out, utf8_bom(&text))?;
        let full = fs::canonicalize(&out).unwrap_or_else(|_| out.clone().into());
        return Ok(
            json!({"exportedUnitsCount": items.len(), "format": format, "outputFile": strip_verbatim(&full.display().to_string())}),
        );
    }
    Ok(Value::Array(items))
}

fn strip_verbatim(p: &str) -> String {
    p.strip_prefix(r"\\?\").unwrap_or(p).to_string()
}
fn pascal(k: &str) -> String {
    let mut c = k.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

pub fn profile_command(
    o: &CliOptions,
    resolve: impl Fn(&CliOptions) -> io::Result<std::path::PathBuf>,
    loc: &Loc,
) -> io::Result<Value> {
    let action = o
        .sub_action
        .clone()
        .unwrap_or_else(|| "schema".into())
        .to_lowercase();
    if action == "schema" {
        let features: Vec<Value> = FeatureRegistry::ALL.iter().map(|f| {
            let tip = format!("{}Tip", f.id);
            json!({"id": f.id, "category": format!("{:?}", f.category), "controlKind": format!("{:?}", f.control_kind), "name": loc.get(f.id), "description": loc.get(&tip)})
        }).collect();
        return Ok(json!({"type": "AgainstRome.PatchProfile", "properties": {
            "features": "Dictionary<string, bool> - 開關類修改項目", "gameSpeed": "int (1..10) - 遊戲加速倍率",
            "villageGarrisonQuotaMultiplier": "int (1..10) - 村莊駐軍配額倍率", "balance": "bool - 兵種平衡模式",
            "customUnitStats": "Dictionary<string, double[]> - 自訂兵種屬性 [HP, Dmg, VW, AW, Speed, Sight, Relt, Range, SpellRadius]"},
            "availableFeatures": features}));
    }
    if action == "export" {
        let out = o
            .output
            .clone()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| arg_error("匯出設定檔必須指定 --output <檔案路徑>。"))?;
        let (game, b) = game_with_backup(o, resolve)?;
        let p = PatchEngine::detect_current_patch_state(&game, &b);
        let dto = profile_to_json(&p);
        let features: Vec<(String, Ordered)> = FeatureRegistry::toggle_features()
            .map(|f| (f.id.to_string(), Ordered::Val(json!(p.get_bool(f.id)))))
            .collect();
        let enabled = features
            .iter()
            .filter(|(_, v)| matches!(v, Ordered::Val(Value::Bool(true))))
            .count();
        let custom = match &p.custom_unit_stats {
            Some(m) => Ordered::Obj(
                m.iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            Ordered::Arr(v.iter().map(|x| Ordered::Val(num(*x))).collect()),
                        )
                    })
                    .collect(),
            ),
            None => Ordered::Val(Value::Null),
        };
        let text = Ordered::Obj(vec![
            ("Features".into(), Ordered::Obj(features)),
            ("GameSpeed".into(), Ordered::Val(dto["GameSpeed"].clone())),
            (
                "VillageGarrisonQuotaMultiplier".into(),
                Ordered::Val(dto["VillageGarrisonQuotaMultiplier"].clone()),
            ),
            ("Balance".into(), Ordered::Val(dto["Balance"].clone())),
            ("CustomUnitStats".into(), custom),
        ])
        .render();
        fs::write(&out, utf8_bom(&text))?;
        let full = fs::canonicalize(&out).unwrap_or_else(|_| out.clone().into());
        return Ok(
            json!({"exported": true, "outputFile": strip_verbatim(&full.display().to_string()), "enabledFeaturesCount": enabled}),
        );
    }
    Err(arg_error(format!(
        "未知的 profile 子指令: '{action}'。支援: schema, export。"
    )))
}

pub fn saves_command(
    o: &CliOptions,
    resolve: impl Fn(&CliOptions) -> io::Result<std::path::PathBuf>,
) -> io::Result<Value> {
    let action = o
        .sub_action
        .as_deref()
        .unwrap_or("list")
        .to_ascii_lowercase();
    if ![
        "list",
        "backup",
        "repair-ai",
        "restore",
        "delete",
        "delete-backup",
        "preview",
        "preview-backup",
    ]
    .contains(&action.as_str())
    {
        return Err(arg_error(format!("未知的 saves 子指令: '{action}'。")));
    }
    // Validate arguments before resolving paths, scanning or creating any directories.
    if action != "list" {
        let name = o
            .sub_arg
            .as_deref()
            .ok_or_else(|| arg_error(format!("saves {action} 必須指定存檔目錄或備份檔名。")))?;
        if !SaveBackupService::is_simple_name(name) {
            return Err(arg_error("存檔目錄或備份檔名無效。"));
        }
        if o.dry_run && !["preview", "preview-backup"].contains(&action.as_str()) {
            return Err(arg_error("saves 操作尚不支援 --dry-run，已取消操作。"));
        }
    }
    if action != "restore" && o.sub_target.is_some() {
        return Err(arg_error("saves 子指令有多餘的位置參數，已取消操作。"));
    }
    if action == "restore" {
        let target = o.sub_target.as_deref().ok_or_else(|| {
            arg_error("使用 saves restore <backup.zip> <folder> 指定還原目的地。")
        })?;
        if !SaveBackupService::is_simple_name(target) {
            return Err(arg_error("還原目的地名稱無效。"));
        }
    }
    let game = resolve(o)?;
    let service = SaveBackupService::new(&game, &game.join("SAVE_BACKUP"))?;
    let mut operation = Value::Null;
    match action.as_str() {
        "backup" => {
            let folder = o.sub_arg.clone().ok_or_else(|| {
                arg_error("請指定欲備份的存檔目錄名稱 (例如: saves backup ESAVE_000)。")
            })?;
            let scan = service.scan()?;
            let info = scan
                .saves
                .iter()
                .find(|s| s.folder.eq_ignore_ascii_case(&folder));
            let file = service.create_backup(
                &folder,
                info.map(|s| s.title.as_str()).unwrap_or(&folder),
                info.map(|s| s.level.as_str()).unwrap_or(""),
            )?;
            operation = json!({"action": action, "fileName": file});
        }
        "repair-ai" => {
            let folder = o.sub_arg.clone().ok_or_else(|| {
                arg_error("請指定欲修復 AI 的存檔目錄名稱 (例如: saves repair-ai ESAVE_000)。")
            })?;
            let result = service.repair_endless_ai(&folder)?;
            operation = json!({"action": action, "result": result});
        }
        "restore" => {
            let warnings = service.restore_backup(
                o.sub_arg.as_deref().unwrap(),
                o.sub_target.as_deref().unwrap(),
            )?;
            operation =
                json!({"action": action, "folder": o.sub_target, "cleanupWarnings": warnings});
        }
        "delete" => {
            service.delete_save(o.sub_arg.as_deref().unwrap())?;
            operation = json!({"action": action, "folder": o.sub_arg});
        }
        "delete-backup" => {
            service.delete_backup(o.sub_arg.as_deref().unwrap())?;
            operation = json!({"action": action, "fileName": o.sub_arg});
        }
        "preview" | "preview-backup" => {
            let image = if action == "preview" {
                service.decode_save_preview(o.sub_arg.as_deref().unwrap())?
            } else {
                service.decode_backup_preview(o.sub_arg.as_deref().unwrap())?
            };
            return Ok(match image {
                Some(i) => {
                    json!({"action": action, "available": true, "width": i.width, "height": i.height})
                }
                None => json!({"action": action, "available": false}),
            });
        }
        _ => {}
    }
    let c = service.scan()?;
    let mut value = json!({
        "saves": c.saves.iter().map(|s| json!({"folder": s.folder, "title": s.title, "level": s.level, "lastWriteTime": s.last_write_time, "parsed": s.parsed})).collect::<Vec<_>>(),
        "backups": c.backups.iter().map(|b| json!({"fileName": b.file_name, "title": b.title, "level": b.level, "backupTime": b.backup_time,
            "origFolder": b.orig_folder, "lastWriteTime": b.last_write_time, "parsed": b.parsed})).collect::<Vec<_>>(),
    });
    if !operation.is_null() {
        value["operation"] = operation;
    }
    Ok(value)
}

pub fn help_json() -> Value {
    let cmd = |name: &str, description: &str, options: &[&str], sub: Option<&[&str]>| {
        let mut v = json!({"name": name, "description": description, "options": options});
        if let Some(s) = sub {
            v["subcommands"] = json!(s);
        }
        v
    };
    json!({"appName": "Against Rome Modifier CLI", "version": "1.2.2", "description": "遊戲修改器 CLI 模式，供 AI 代理人與自動化腳本操作",
        "commands": [
            cmd("detect-game", "自動偵測或驗證遊戲目錄路徑", &["--game <path>"], None),
            cmd("status", "檢視遊戲與修改器目前狀態、已套用修改、備份狀態等", &["--game <path>", "--json"], None),
            cmd("features", "列出所有可用的修改功能項目與說明", &["--category <Stats|Compat|Language>", "--game <path>", "--json"], None),
            cmd("apply", "套用修改項目到遊戲檔案", &["--game <path>", "--category <Stats|Compat|Language>", "--compat", "--enable <ids>", "--disable <ids>", "--all", "--disable-all", "--balance",
                "--game-speed <1..10>", "--garrison-quota <1..10>", "--preset <filePath>", "--profile <filePath>", "--dry-run", "--json"], None),
            cmd("restore", "將遊戲檔案還原為原版或備份狀態", &["--game <path>", "--all", "--stats", "--compat", "--language", "--preserve-custom-maps <true|false>", "--json"], None),
            cmd("backup", "管理遊戲原版檔案備份（status / ensure / heal）", &["--game <path>", "--json"], Some(&["status", "ensure", "heal"])),
            cmd("stats", "檢視與匯出兵種數值（list / get / export）", &["--game <path>", "--balance", "--json"], Some(&["list", "get <unitKey>", "export --format <ini|json> --output <path>"])),
            cmd("profile", "匯出或檢視修改配置設定檔（export / schema）", &["--game <path>", "--json"], Some(&["schema", "export --output <path>"])),
            cmd("saves", "管理遊戲存檔、備份與縮圖", &["--game <path>", "--json"], Some(&["list", "backup <folder>", "repair-ai <folder>", "restore <backup.zip> <folder>", "delete <folder>", "delete-backup <backup.zip>", "preview <folder>", "preview-backup <backup.zip>"])),
            cmd("maps", "列出遊戲地圖（list）", &["--game <path>", "--json"], Some(&["list"])),
        ],
        "examples": ["AgainstRomeModifier.exe status --json", "AgainstRomeModifier.exe apply --enable FastCiviProduction,InfiniteMorale --game-speed 3 --json",
            "AgainstRomeModifier.exe apply --all --json", "AgainstRomeModifier.exe restore --all --json", "AgainstRomeModifier.exe features --category Stats --json",
            "AgainstRomeModifier.exe stats get GER_INF01 --json", "AgainstRomeModifier.exe saves list --json"]})
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
fn yn(b: bool, yes: &str, no: &str) -> String {
    if b { yes } else { no }.to_string()
}

/// Text-mode output of each command (CliRunner.RunAsync renderers).
pub fn render_text(command: &str, v: &Value, loc: &Loc) -> String {
    let mut o = String::new();
    macro_rules! line { ($($a:tt)*) => {{ let _ = writeln!(o, $($a)*); }}; }
    match command {
        "detect-game" => {
            line!(
                "遊戲目錄: {}",
                v["detectedPath"].as_str().unwrap_or("(未找到)")
            );
            line!(
                "目錄有效: {}",
                yn(v["isValid"].as_bool().unwrap_or(false), "是", "否")
            );
            line!(
                "包含主程式 Against_Rome.exe: {}",
                yn(v["hasExe"].as_bool().unwrap_or(false), "是", "否")
            );
            if let Some(r) = v["registryPath"].as_str() {
                line!("登錄檔偵測路徑: {r}");
            }
        }
        "status" => {
            line!("遊戲目錄: {}", s(v, "gamePath"));
            line!(
                "備份狀態: {} (缺少 {} 個檔案)",
                yn(
                    v["backup"]["loaded"].as_bool().unwrap_or(false),
                    "已載入",
                    "未載入"
                ),
                v["backup"]["missingCount"]
            );
            line!("遊戲加速倍率: {}x", v["gameSpeed"]);
            line!("村莊駐軍配額: {}x", v["villageGarrisonQuota"]);
            line!(
                "平衡模式: {}",
                yn(v["balance"].as_bool().unwrap_or(false), "開", "關")
            );
            line!(
                "dgVoodoo: {}",
                yn(
                    v["dgVoodooInstalled"].as_bool().unwrap_or(false),
                    "已安裝",
                    "未安裝"
                )
            );
            line!(
                "ArgmTrace: {}",
                yn(
                    v["argmTraceInstalled"].as_bool().unwrap_or(false),
                    "已安裝",
                    "未安裝"
                )
            );
            line!(
                "自訂兵種屬性: {}",
                yn(
                    v["hasCustomUnitStats"].as_bool().unwrap_or(false),
                    "已設定",
                    "無"
                )
            );
            line!("已啟用修改項目 ({} 項):", v["activeFeaturesCount"]);
            let mut active: Vec<&String> = v["features"]
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(_, b)| b.as_bool() == Some(true))
                .map(|(k, _)| k)
                .collect();
            active.sort();
            for k in active {
                line!("  - {k} ({})", loc.get(k));
            }
        }
        "features" => {
            let list = v.as_array().cloned().unwrap_or_default();
            line!("修改功能清單 (共 {} 項):", list.len());
            let mut sorted = list.clone();
            sorted.sort_by(|a, b| {
                (s(a, "category"), s(a, "id")).cmp(&(s(b, "category"), s(b, "id")))
            });
            let mut cat = "";
            for item in &sorted {
                if s(item, "category") != cat {
                    cat = s(item, "category");
                    line!("\n[{cat}]");
                }
                let state = match item["currentValue"].as_bool() {
                    Some(true) => " [已開啟]",
                    Some(false) => " [已關閉]",
                    None => "",
                };
                line!("  - {}: {}{state}", s(item, "id"), s(item, "name"));
                if !s(item, "description").trim().is_empty() {
                    line!("      說明: {}", s(item, "description"));
                }
            }
        }
        "apply" => {
            let prefix = if v["isDryRun"].as_bool().unwrap_or(false) {
                "[DRY-RUN 預覽] "
            } else {
                ""
            };
            line!("{prefix}成功套用修改至: {}", s(v, "gamePath"));
            line!("已啟用項目 ({} 項):", v["appliedFeaturesCount"]);
            let mut ids: Vec<&str> = v["enabledFeatures"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            ids.sort();
            for id in ids {
                line!("  - {id} ({})", loc.get(id));
            }
            line!("遊戲加速: {}x", v["gameSpeed"]);
            line!("駐軍配額: {}x", v["villageGarrisonQuotaMultiplier"]);
            line!(
                "平衡模式: {}",
                yn(v["balance"].as_bool().unwrap_or(false), "開", "關")
            );
            if v["customStatsUnitsCount"].as_i64().unwrap_or(0) > 0 {
                line!("自訂兵種數值: {} 種兵種", v["customStatsUnitsCount"]);
            }
        }
        "restore" => {
            line!("已完成還原遊戲目錄: {}", s(v, "gamePath"));
            let cats: Vec<&str> = v["categoriesRestored"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            line!("還原分類: {}", cats.join(", "));
            line!(
                "保留自訂地圖: {}",
                yn(
                    v["preserveCustomMaps"].as_bool().unwrap_or(true),
                    "是",
                    "否"
                )
            );
        }
        "backup" => {
            line!(
                "備份操作 [{}]: {}",
                s(v, "action"),
                yn(v["success"].as_bool().unwrap_or(false), "成功", "失敗")
            );
            line!("遊戲目錄: {}", s(v, "gamePath"));
            line!("缺少備份檔案數: {}", v["missingCount"]);
            let missing: Vec<&str> = v["missingFiles"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            if !missing.is_empty() {
                line!("缺少的檔案:");
                for f in missing {
                    line!("  - {f}");
                }
            }
        }
        "stats" => {
            if let Some(list) = v.as_array() {
                line!("兵種數值列表 (共 {} 筆):", list.len());
                for u in list {
                    line!(
                        "  [{}] {} (HP:{}, Dmg:{}, Def:{}, Spd:{}, Sight:{}, Rng:{})",
                        s(u, "key"),
                        s(u, "name"),
                        u["hp"],
                        u["damage"],
                        u["defenseVw"],
                        u["speed"],
                        u["sight"],
                        u["range"]
                    );
                }
            } else if v.get("key").is_some() {
                line!(
                    "兵種: {} ({}) - {} {} {}",
                    s(v, "key"),
                    s(v, "name"),
                    s(v, "faction"),
                    s(v, "tier"),
                    s(v, "unitType")
                );
                line!(
                    "生命: {}, 近戰/遠程傷害: {}, 防禦(VW): {}, 戰鬥(AW): {}",
                    v["hp"],
                    v["damage"],
                    v["defenseVw"],
                    v["combatAw"]
                );
                line!(
                    "速度: {}, 視野: {}, 冷卻: {}, 射程: {}, 法術半徑: {}",
                    v["speed"],
                    v["sight"],
                    v["reloadRelt"],
                    v["range"],
                    v["spellRadius"]
                );
            } else {
                line!("{}", serde_json::to_string_pretty(v).unwrap());
            }
        }
        "saves" => {
            if v.get("available").is_some() {
                if v["available"] == true {
                    line!("存檔縮圖: {} x {}", v["width"], v["height"]);
                } else {
                    line!("無可用的存檔縮圖。");
                }
                return o;
            }
            if !v["operation"].is_null() {
                line!("存檔操作: {}", v["operation"]);
            }
            let saves = v["saves"].as_array().cloned().unwrap_or_default();
            line!("遊戲存檔 (共 {} 個):", saves.len());
            for x in &saves {
                line!(
                    "  - {}: {} ({}) [{}]",
                    s(x, "folder"),
                    s(x, "title"),
                    s(x, "level"),
                    s(x, "lastWriteTime")
                        .get(..16)
                        .unwrap_or("")
                        .replace('T', " ")
                );
            }
            let backups = v["backups"].as_array().cloned().unwrap_or_default();
            line!("備份存檔 (共 {} 個):", backups.len());
            for x in &backups {
                line!(
                    "  - {}: {} [{}]",
                    s(x, "fileName"),
                    s(x, "title"),
                    s(x, "backupTime")
                );
            }
        }
        "maps" => {
            let maps = v.as_array().cloned().unwrap_or_default();
            line!("地圖列表 (共 {} 張):", maps.len());
            for m in &maps {
                let custom = if m["isCustom"].as_bool().unwrap_or(false) {
                    " [自訂/可編輯]"
                } else {
                    ""
                };
                line!(
                    "  - {}: {} ({}){custom}",
                    s(m, "id"),
                    m["displayName"].as_str().unwrap_or(s(m, "id")),
                    s(m, "category")
                );
            }
        }
        "help" => {
            line!("=== {} v{} ===", s(v, "appName"), s(v, "version"));
            line!("{}\n", s(v, "description"));
            line!("可用指令 (Commands):");
            for c in v["commands"].as_array().into_iter().flatten() {
                line!("  {:<15} {}", s(c, "name"), s(c, "description"));
                let subs: Vec<&str> = c["subcommands"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                if !subs.is_empty() {
                    line!("    子指令: {}", subs.join(", "));
                }
                let opts: Vec<&str> = c["options"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                if !opts.is_empty() {
                    line!("    選項: {}", opts.join(" "));
                }
            }
            line!("\n使用範例 (Examples):");
            for e in v["examples"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                line!("  {e}");
            }
        }
        _ => {
            line!("{}", serde_json::to_string_pretty(v).unwrap());
        }
    }
    o
}

#[allow(dead_code)]
fn _keep(_: &Map<String, Value>, _: &PatchProfile) {}
