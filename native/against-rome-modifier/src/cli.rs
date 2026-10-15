//! Modifier command line contract, ported from src.Modifier/Cli.
use crate::{FeatureRegistry, FeatureControlKind, PatchProfile, TroopConfig};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, io, path::{Path, PathBuf}};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CliOptions {
    pub command: String, pub sub_action: Option<String>, pub sub_arg: Option<String>,
    pub game_path: Option<String>, pub json: bool, pub verbose: bool,
    pub enable: Vec<String>, pub disable: Vec<String>, pub enable_all: bool, pub disable_all: bool,
    pub balance: Option<bool>, pub game_speed: Option<i32>, pub quota: Option<i32>,
    pub preset: Option<String>, pub profile: Option<String>, pub dry_run: bool,
    pub restore_all: bool, pub restore_stats: bool, pub restore_compat: bool, pub compat: bool,
    pub restore_language: bool, pub preserve_custom_maps: bool,
    pub output: Option<String>, pub format: Option<String>, pub category: Option<String>, pub language: Option<String>,
}
fn canonical_command(s: &str) -> Option<&'static str> {
    Some(match s.to_ascii_lowercase().as_str() {
        "detect-game"|"game"=>"detect-game", "status"|"detect"|"info"=>"status",
        "features"|"list"=>"features", "apply"=>"apply", "restore"=>"restore", "backup"=>"backup",
        "stats"=>"stats", "profile"=>"profile", "saves"=>"saves", "maps"=>"maps", "help"=>"help", _=>return None,
    })
}
impl CliOptions {
    pub fn parse(args: &[String]) -> Result<Self,String> {
        let mut o=Self { command:"help".into(),preserve_custom_maps:true,..Self::default() };
        let mut pos=Vec::new(); let mut i=0;
        while i<args.len() {
            let arg=args[i].to_ascii_lowercase();
            let mut value=|| -> Result<String,String> { i+=1; args.get(i).cloned().ok_or_else(||format!("{} 缺少參數。",arg)) };
            match arg.as_str() {
                "--cli"=>{}, "--json"=>o.json=true, "--verbose"|"-v"=>o.verbose=true,
                "--game"|"-g"=>o.game_path=Some(value()?),
                "--enable"|"--disable"=>{ let v=value()?; let target=if arg=="--enable" {&mut o.enable} else {&mut o.disable}; target.extend(v.split([',',';']).filter(|x|!x.is_empty()).map(|x|x.trim().into())); },
                "--all"=>{o.enable_all=true;o.restore_all=true;}, "--disable-all"=>o.disable_all=true,
                "--balance"|"--preserve-custom-maps"=>{ let mut b=true; if let Some(v)=args.get(i+1) { if v.eq_ignore_ascii_case("true")||v.eq_ignore_ascii_case("false") { b=v.eq_ignore_ascii_case("true");i+=1; } } if arg=="--balance" {o.balance=Some(b)} else {o.preserve_custom_maps=b} },
                "--game-speed"|"--speed"|"--garrison-quota"|"--quota"=>{let n=value()?.parse::<i32>().map_err(|_|format!("{} 缺少有效的數值倍率 (1..10)。",arg))?; if !(1..=10).contains(&n) {return Err("倍率必須介於 1 到 10 之間。".into());} if arg=="--speed"||arg=="--game-speed" {o.game_speed=Some(n)} else {o.quota=Some(n)} },
                "--preset"=>o.preset=Some(value()?), "--profile"=>o.profile=Some(value()?),
                "--dry-run"=>o.dry_run=true, "--stats"=>o.restore_stats=true,
                "--compat"=>{o.compat=true;o.restore_compat=true;}, "--language"|"--lang-pack"=>o.restore_language=true,
                "--delete-custom-maps"=>o.preserve_custom_maps=false,
                "--output"|"-o"=>o.output=Some(value()?), "--format"|"-f"=>o.format=Some(value()?),
                "--category"|"-c"=>o.category=Some(value()?), "--unit"|"-u"=>o.sub_arg=Some(value()?), "--lang"=>o.language=Some(value()?),
                "--help"|"-h"|"/?"=>return Ok(o),
                _=>if !args[i].starts_with('-') {pos.push(args[i].clone());},
            } i+=1;
        }
        if let Some(c)=pos.first() { o.command=canonical_command(c).ok_or_else(||format!("未知的指令: '{c}'。使用 'AgainstRomeModifier.exe help' 檢視說明。"))?.into(); }
        o.sub_action=pos.get(1).cloned(); if o.sub_arg.is_none() {o.sub_arg=pos.get(2).cloned();} Ok(o)
    }
}
pub const ENABLE_ALL_EXCLUDED: &[&str]=&["Balance","CameraZoomOut1","CorpseRetention","ArgmTrace","SpellHealing10x","SpellResurrection","GeneralSkills","LeaderGlory"];
fn invalid(s:impl Into<String>)->io::Error {io::Error::new(io::ErrorKind::InvalidInput,s.into())}
fn read_json(path:&str)->io::Result<Value> {serde_json::from_slice(&fs::read(path)?).map_err(|e|invalid(e.to_string()))}
pub fn profile_to_json(p:&PatchProfile)->Value {
    let features:BTreeMap<_,_>=FeatureRegistry::toggle_features().map(|f|(f.id,p.get_bool(f.id))).collect();
    json!({"Features":features,"GameSpeed":p.get_int("GameSpeed"),"VillageGarrisonQuotaMultiplier":p.get_int("VillageGarrisonQuotaMultiplier"),"Balance":p.get_bool("Balance"),"CustomUnitStats":p.custom_unit_stats})
}
pub fn profile_from_json(v:&Value)->io::Result<PatchProfile> {
    if !v.is_object() {return Err(invalid("設定檔格式無效。"));}
    let mut p=PatchProfile::new();
    if let Some(obj)=v.get("Features") {let obj=obj.as_object().ok_or_else(||invalid("Features must be an object"))?; for (id,value) in obj {p.set_bool(id,value.as_bool().ok_or_else(||invalid("Feature values must be boolean"))?);} }
    for id in ["GameSpeed","VillageGarrisonQuotaMultiplier"] { if let Some(n)=v.get(id) {p.set_int(id,i32::try_from(n.as_i64().ok_or_else(||invalid("Expected integer"))?).map_err(|_|invalid("Integer overflow"))?);} }
    if let Some(b)=v.get("Balance") {p.set_bool("Balance",b.as_bool().ok_or_else(||invalid("Expected boolean"))?);}
    if let Some(stats)=v.get("CustomUnitStats").filter(|x|!x.is_null()) {p.custom_unit_stats=Some(serde_json::from_value(stats.clone()).map_err(|e|invalid(e.to_string()))?);} Ok(p)
}
/// Preserves the six-column legacy preset mapping, including its zero speed.
pub fn parse_preset(text:&str, fallback:impl Fn(&str)->Vec<f64>)->(BTreeMap<String,Vec<f64>>,usize) {
    let mut out=BTreeMap::new();let mut skipped=0;
    for line in text.lines().map(str::trim) {if line.is_empty()||line.starts_with(['#',';']) {continue;} let Some((key,tail))=line.split_once('=') else {continue}; if key.trim().is_empty() {continue;}
        let values:Vec<_>=tail.split(',').collect();if values.len()<4 {skipped+=1;continue;}
        let parsed:Result<Vec<f64>,_>=values.iter().take(9).map(|s|s.trim().parse::<f64>()).collect(); let Ok(parsed)=parsed else {skipped+=1;continue;};
        let mut a=vec![0.;9];a[..4].copy_from_slice(&parsed[..4]);
        if values.len()==6 {a[5]=parsed[4];a[6]=parsed[5];} else {
            for j in 5..9 {a[j]=parsed.get(j).copied().unwrap_or(0.);}
            if values.len()<9 {let base=fallback(key.trim()); for j in 4..9 {if values.len()<=j {a[j]=base.get(j).copied().unwrap_or(0.);}}}
        }
        let key=key.trim();if let Some(old)=out.keys().find(|k:&&String|k.eq_ignore_ascii_case(key)).cloned(){out.remove(&old);}out.insert(key.into(),a);
    }(out,skipped)
}
pub fn apply_options(o:&CliOptions,mut p:PatchProfile)->io::Result<PatchProfile> {
    if let Some(c)=&o.category {if !["Stats","Compat","Language"].iter().any(|s|s.eq_ignore_ascii_case(c)) {return Err(invalid(format!("未知的修改分類: '{c}'。")));}}
    for id in o.enable.iter().chain(o.disable.iter()) {if FeatureRegistry::get(id).is_none(){return Err(invalid(format!("未知的修改功能 ID: '{id}'。")));}}
    if let Some(path)=&o.profile {return profile_from_json(&read_json(path)?);}
    if o.disable_all {p=PatchProfile::new();}
    let category=o.category.as_deref().or(if o.compat {Some("Compat")}else{None});
    for f in FeatureRegistry::toggle_features() {let excluded=ENABLE_ALL_EXCLUDED.contains(&f.id); if let Some(c)=category {if format!("{:?}",f.category).eq_ignore_ascii_case(c)&&(o.enable_all||!excluded) {p.set_bool(f.id,true);}} else if o.enable_all&&!excluded {p.set_bool(f.id,true);} }
    for id in &o.enable {p.set_bool(FeatureRegistry::get(id).unwrap().id,true);}for id in &o.disable {p.set_bool(FeatureRegistry::get(id).unwrap().id,false);}
    if let Some(b)=o.balance {p.set_bool("Balance",b);}if let Some(n)=o.game_speed {p.set_int("GameSpeed",n);}if let Some(n)=o.quota {p.set_int("VillageGarrisonQuotaMultiplier",n);} Ok(p)
}

fn resolve_game(o:&CliOptions)->io::Result<PathBuf> {
    let candidate=if let Some(p)=o.game_path.as_ref().filter(|s|!s.trim().is_empty()) {PathBuf::from(p)} else {crate::services::GameDirectoryLocator::resolve_initial_game_path(None)};
    if !candidate.is_dir(){return Err(io::Error::new(io::ErrorKind::NotFound,format!("指定的遊戲目錄不存在: {}",candidate.display())));}fs::canonicalize(candidate)
}
fn validate_exe(game:&Path)->io::Result<()> {if !game.join("Against_Rome.exe").is_file(){return Err(io::Error::new(io::ErrorKind::NotFound,"遊戲目錄下未找到主執行檔 Against_Rome.exe"));}Ok(())}
pub fn features_json(o:&CliOptions,p:Option<&PatchProfile>)->Value {
    Value::Array(FeatureRegistry::ALL.iter().filter(|f|o.category.as_ref().map(|c|format!("{:?}",f.category).eq_ignore_ascii_case(c)).unwrap_or(true)).map(|f|{
        let mut v=json!({"id":f.id,"name":f.id,"description":format!("{}Tip",f.id),"category":format!("{:?}",f.category),"controlKind":format!("{:?}",f.control_kind),"isExcludedFromEnableAll":ENABLE_ALL_EXCLUDED.contains(&f.id)});
        if let Some(p)=p.filter(|_|f.control_kind==FeatureControlKind::Toggle){v["currentValue"]=json!(p.get_bool(f.id));} v
    }).collect())
}

pub fn maps_json(game:&Path)->io::Result<Value> {
    let root=game.join("MAPS");if !root.is_dir(){return Ok(json!([]));}
    let mut maps=Vec::new();for entry in fs::read_dir(root)? {let e=entry?;let id=e.file_name().to_string_lossy().into_owned();let p=e.path();if !p.is_dir()||!id.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'_')||!p.join("boden.txt").is_file()||!p.join("minimap.bmp").is_file(){continue;}
        let upper=id.to_ascii_uppercase();let (order,category)=if upper.starts_with("KAMP_"){(0,"劇情戰役")}else if upper.starts_with("HIST_"){(1,"歷史戰役")}else if upper.starts_with("TUTOR_"){(2,"教學")}else if upper.starts_with("ENDL_"){(3,"無盡模式")}else if upper.starts_with("MP_"){(4,"多人地圖")}else{(5,"其他地圖")};
        let slot=upper.strip_prefix("ENDL_").and_then(|s|s.parse::<i32>().ok());let custom=upper.len()==8&&slot.is_some_and(|n|(5..=999).contains(&n))&&p.join(".arm_custom_map").is_file();
        let mut v=json!({"id":id,"category":category,"isCustom":custom});if let Some(n)=slot {v["endlessSlot"]=json!(n);}
        let title=p.join("TEXT/US/briefing.put");if let Ok(b)=fs::read(title) {let text=crate::decode_cp1251(&crate::decompress_if_pfil(&b).unwrap_or(b));for line in text.lines(){if let Some((k,vv))=line.split_once('='){if k.trim().eq_ignore_ascii_case("briefing_titel_1"){v["displayName"]=json!(vv.trim().trim_matches('"'));}}}}
        maps.push((order,upper,v));
    }maps.sort_by(|a,b|(a.0,&a.1).cmp(&(b.0,&b.1)));Ok(Value::Array(maps.into_iter().map(|x|x.2).collect()))
}


pub fn is_cli_invocation(args:&[String])->bool {
    let mut i=0;
    while i<args.len() {
        let a=args[i].to_ascii_lowercase();
        if ["--cli","--help","-h","/?","--version","-v","--json"].contains(&a.as_str()) {return true;}
        if a.starts_with('-') {
            if ["--game","-g","--map","--preset","--profile","--output","-o","--format","-f","--category","-c","--lang","--unit","-u","--enable","--disable","--game-speed","--speed","--garrison-quota","--quota"].contains(&a.as_str()) {i+=1;}
        } else if canonical_command(&a).is_some() {return true;}
        i+=1;
    }
    false
}

/// Writes every planned file via temp+rename; on any failure restores the originals already touched.
pub fn commit_plan(plan:&crate::services::PatchFilePlan)->io::Result<()> {
    let mut undo:Vec<(PathBuf,Option<Vec<u8>>)>=Vec::new();
    let result=(||->io::Result<()> {
        for (path,bytes) in &plan.files {
            undo.push((path.clone(),fs::read(path).ok()));
            if let Some(dir)=path.parent() {fs::create_dir_all(dir)?;}
            let tmp=PathBuf::from(format!("{}.arm-tmp",path.display()));
            fs::write(&tmp,bytes)?;
            if let Err(e)=fs::rename(&tmp,path) {let _=fs::remove_file(&tmp);return Err(e);}
        }
        for path in &plan.deletes {
            if path.is_file() {undo.push((path.clone(),fs::read(path).ok()));fs::remove_file(path)?;}
        }
        Ok(())
    })();
    if let Err(e)=result {
        for (path,old) in undo.into_iter().rev() {match old {Some(b)=>{let _=fs::write(&path,b);},None=>{let _=fs::remove_file(&path);}}}
        return Err(e);
    }
    Ok(())
}

fn load_backup(game:&Path)->io::Result<crate::services::BackupManager> {
    let mut b=crate::services::BackupManager::new();
    let base=std::env::current_exe().ok().and_then(|p|p.parent().map(Path::to_path_buf)).unwrap_or_default();
    b.load_backup_zip_to_memory(game,None,&base)?;
    Ok(b)
}

fn envelope(command:&str,data:Result<Value,String>)->Value {
    match data {Ok(d)=>json!({"success":true,"command":command,"data":d}),Err(e)=>json!({"success":false,"command":command,"error":e})}
}

pub const HELP: &str="Against Rome Modifier CLI\n指令: detect-game, status, features, apply, restore, profile, maps, help\n選項: --game <路徑> --json --enable a,b --disable a --all --disable-all --category Stats|Compat|Language --dry-run";

/// Runs a parsed command line; returns the process exit code (0 ok, 1 runtime failure, 2 argument error).
pub fn run_cli(args:&[String],out:&mut dyn io::Write,err:&mut dyn io::Write)->i32 {
    let o=match CliOptions::parse(args) {
        Ok(o)=>o,
        Err(e)=>{
            if args.iter().any(|a|a.eq_ignore_ascii_case("--json")) {let _=writeln!(out,"{}",serde_json::to_string_pretty(&envelope("unknown",Err(e))).unwrap());}
            else {let _=writeln!(err,"參數錯誤: {e}");let _=writeln!(err,"執行 'AgainstRomeModifier.exe help' 可檢視說明。");}
            return 2;
        }
    };
    let name=o.command.clone();
    let result:Result<Value,String>=match name.as_str() {
        "help"=>{let _=writeln!(out,"{HELP}");return 0;}
        "detect-game"=>Ok(detect_game_json(&o)),
        "status"=>status_json(&o).map_err(|e|e.to_string()),
        "features"=>features_command(&o).map_err(|e|e.to_string()),
        "maps"=>resolve_game(&o).and_then(|g|maps_json(&g)).map_err(|e|e.to_string()),
        "profile"=>resolve_game(&o).and_then(|g|{validate_exe(&g)?;let b=load_backup(&g)?;Ok(profile_to_json(&crate::services::PatchEngine::detect_current_patch_state(&g,&b)))}).map_err(|e|e.to_string()),
        "apply"=>apply_command(&o).map_err(|e|e.to_string()),
        "restore"=>restore_command(&o).map_err(|e|e.to_string()),
        other=>Err(format!("指令 '{other}' 尚未移植到 Rust CLI。")),
    };
    let ok=result.is_ok();
    if o.json {let _=writeln!(out,"{}",serde_json::to_string_pretty(&envelope(&name,result)).unwrap());}
    else {match result {Ok(v)=>{let _=writeln!(out,"{}",serde_json::to_string_pretty(&v).unwrap());},Err(e)=>{let _=writeln!(err,"錯誤: {e}");}}}
    if ok {0} else {1}
}
fn detect_game_json(o:&CliOptions)->Value {
    use crate::services::GameDirectoryLocator as L;
    let detected=L::resolve_initial_game_path(o.game_path.as_deref().map(Path::new));
    let valid=!detected.as_os_str().is_empty()&&detected.is_dir();
    let registry=L::detect_from_registry();
    let mut v=json!({"isValid":valid,"hasExe":valid&&detected.join("Against_Rome.exe").is_file(),"defaultPath":crate::services::DEFAULT_INSTALL_PATH});
    if valid {v["detectedPath"]=json!(fs::canonicalize(&detected).unwrap_or(detected).display().to_string());}
    if !registry.as_os_str().is_empty() {v["registryPath"]=json!(registry.display().to_string());}
    v
}
fn status_json(o:&CliOptions)->io::Result<Value> {
    let game=resolve_game(o)?;validate_exe(&game)?;
    let b=load_backup(&game)?;let missing=b.find_missing_backup_resources();
    let p=crate::services::PatchEngine::detect_current_patch_state(&game,&b);
    let features:BTreeMap<_,_>=FeatureRegistry::toggle_features().map(|f|(f.id,p.get_bool(f.id))).collect();
    Ok(json!({"gamePath":game.display().to_string(),"backup":{"loaded":!b.backup_files().is_empty(),"missingCount":missing.len(),"missingFiles":missing},
        "gameSpeed":p.get_int("GameSpeed"),"villageGarrisonQuota":p.get_int("VillageGarrisonQuotaMultiplier"),
        "dgVoodooInstalled":crate::services::PatchEngine::is_dgvoodoo_installed(&game),"argmTraceInstalled":crate::services::PatchEngine::is_argm_trace_installed(&game),
        "hasCustomUnitStats":p.custom_unit_stats.as_ref().is_some_and(|m|!m.is_empty()),"balance":p.get_bool("Balance"),
        "activeFeaturesCount":features.values().filter(|v|**v).count(),"features":features}))
}
fn features_command(o:&CliOptions)->io::Result<Value> {
    let mut current=None;
    if let Ok(game)=resolve_game(o) {if game.join("Against_Rome.exe").is_file() {if let Ok(b)=load_backup(&game) {current=Some(crate::services::PatchEngine::detect_current_patch_state(&game,&b));}}}
    Ok(features_json(o,current.as_ref()))
}
fn apply_command(o:&CliOptions)->io::Result<Value> {
    if let Some(c)=&o.category {if !["Stats","Compat","Language"].iter().any(|s|s.eq_ignore_ascii_case(c)) {return Err(invalid(format!("未知的修改分類: '{c}'。")));}}
    if let Some(path)=&o.profile {if !Path::new(path).is_file() {return Err(io::Error::new(io::ErrorKind::NotFound,format!("找不到設定檔: {path}")));}}
    let game=resolve_game(o)?;validate_exe(&game)?;
    let mut b=load_backup(&game)?;
    if !b.ensure_backup_loaded_for_game_path(&game)? {return Err(invalid("無法載入或建立遊戲備份基準。請先確認遊戲目錄包含原版檔案。"));}
    let start=if o.disable_all||o.profile.is_some() {PatchProfile::new()} else {crate::services::PatchEngine::detect_current_patch_state(&game,&b)};
    let mut profile=apply_options(o,start)?;
    if o.disable_all&&o.profile.is_none() {
        if o.game_speed.is_none() {profile.set_int("GameSpeed",1);}
        if o.quota.is_none() {profile.set_int("VillageGarrisonQuotaMultiplier",1);}
    }
    let plan=crate::services::PatchEngine::compose(&game,&profile,&b)?;
    if !o.dry_run {commit_plan(&plan)?;}
    let enabled:Vec<_>=FeatureRegistry::toggle_features().filter(|f|profile.get_bool(f.id)).map(|f|f.id).collect();
    Ok(json!({"gamePath":game.display().to_string(),"isDryRun":o.dry_run,"appliedFeaturesCount":enabled.len(),"enabledFeatures":enabled,
        "gameSpeed":profile.get_int("GameSpeed"),"villageGarrisonQuotaMultiplier":profile.get_int("VillageGarrisonQuotaMultiplier"),"balance":profile.get_bool("Balance"),
        "plannedFiles":plan.files.len()}))
}
fn restore_command(o:&CliOptions)->io::Result<Value> {
    use crate::services::{PatchEngine,PatchFilePlan};
    let game=resolve_game(o)?;validate_exe(&game)?;
    let b=load_backup(&game)?;
    let specific=o.restore_stats||o.restore_compat||o.restore_language;
    let all=o.restore_all||!specific;
    let mut cats=Vec::new();let mut plan=PatchFilePlan::default();
    let mut merge=|p:PatchFilePlan|{plan.files.extend(p.files);plan.deletes.extend(p.deletes);};
    if all {merge(PatchEngine::restore_original_files(&game,&b,o.preserve_custom_maps)?);cats=vec!["Stats","Compat","Language"];}
    else {
        if o.restore_stats {merge(PatchEngine::restore_stats_only(&game,&b)?);cats.push("Stats");}
        if o.restore_compat {merge(PatchEngine::restore_compat_only(&game,&b)?);cats.push("Compat");}
        if o.restore_language {merge(PatchEngine::restore_language_only(&game)?);cats.push("Language");}
    }
    if !o.dry_run {commit_plan(&plan)?;}
    Ok(json!({"gamePath":game.display().to_string(),"categoriesRestored":cats,"preserveCustomMaps":o.preserve_custom_maps,"isDryRun":o.dry_run,"plannedFiles":plan.files.len()}))
}

#[cfg(test)] mod tests {
    use super::*;
    fn args(a:&[&str])->Vec<String>{a.iter().map(|s|s.to_string()).collect()}
    #[test]fn parser_aliases_and_precedence(){let o=CliOptions::parse(&args(&["--cli","apply","--enable","a,b;c","--all","--balance","false","--quota","10","--unit","x","--json"])).unwrap();assert_eq!(o.enable,["a","b","c"]);assert_eq!(o.balance,Some(false));assert_eq!(o.quota,Some(10));assert!(o.enable_all&&o.restore_all&&o.json);}
    #[test]fn parser_missing_range_and_unknown_command(){for a in [vec!["apply","--game"],vec!["apply","--speed","11"],vec!["wat"]]{assert!(CliOptions::parse(&args(&a)).is_err());}}
    #[test]fn legacy_preset_and_fallback(){let (v,n)=parse_preset("u=1,2,3,4,50,60\nv=1,2,3,4\nbad=1,2\n", |_|vec![7.;9]);assert_eq!(n,1);assert_eq!(v["u"],vec![1.,2.,3.,4.,0.,50.,60.,0.,0.]);assert_eq!(&v["v"][4..],&[7.;5]);}
    #[test]fn all_exclusions_category_override_and_disable_order(){let o=CliOptions::parse(&args(&["apply","--all","--disable","FocusLoss"])).unwrap();let p=apply_options(&o,PatchProfile::new()).unwrap();assert!(!p.get_bool("FocusLoss"));assert!(!p.get_bool("ArgmTrace"));let o=CliOptions::parse(&args(&["apply","--category","Compat","--all"])).unwrap();assert!(apply_options(&o,p).unwrap().get_bool("ArgmTrace"));}
    #[test]fn profile_pascal_case_roundtrip_and_case_replacement(){let mut p=PatchProfile::new();p.set_bool("FocusLoss",true);p.set_bool("focusloss",false);assert!(!p.get_bool("FocusLoss"));let p=profile_from_json(&profile_to_json(&p)).unwrap();assert_eq!(p.get_int("GameSpeed"),1);}
    #[test]fn cli_invocation_detection(){assert!(is_cli_invocation(&args(&["status"])));assert!(is_cli_invocation(&args(&["--game","x","--json"])));assert!(!is_cli_invocation(&args(&["--game","status"])));assert!(!is_cli_invocation(&[]));}
    #[test]fn run_cli_error_paths_and_help(){
        let(mut o,mut e)=(Vec::new(),Vec::new());assert_eq!(run_cli(&args(&["wat","--json"]),&mut o,&mut e),2);assert!(String::from_utf8(o).unwrap().contains("\"success\": false"));
        let(mut o,mut e)=(Vec::new(),Vec::new());assert_eq!(run_cli(&args(&["help"]),&mut o,&mut e),0);assert!(!o.is_empty());
        let(mut o,mut e)=(Vec::new(),Vec::new());assert_eq!(run_cli(&args(&["status","--game","Z:/nonexistent-arm"]),&mut o,&mut e),1);assert!(!e.is_empty());
    }
    #[test]fn commit_plan_writes_deletes_and_rolls_back(){
        let d=std::env::temp_dir().join(format!("arm-cli-commit-{}",std::process::id()));let _=fs::remove_dir_all(&d);fs::create_dir_all(&d).unwrap();
        let (a,b,c)=(d.join("a.bin"),d.join("b.bin"),d.join("c.bin"));fs::write(&a,b"old").unwrap();fs::write(&c,b"del").unwrap();
        let mut plan=crate::services::PatchFilePlan::default();plan.files.insert(a.clone(),b"new".to_vec());plan.files.insert(b.clone(),b"b".to_vec());plan.deletes.push(c.clone());
        commit_plan(&plan).unwrap();assert_eq!(fs::read(&a).unwrap(),b"new");assert!(b.is_file()&&!c.is_file());
        let bad=d.join("dir");fs::create_dir(&bad).unwrap();let mut plan=crate::services::PatchFilePlan::default();plan.files.insert(a.clone(),b"zzz".to_vec());plan.files.insert(bad.clone(),b"x".to_vec());
        assert!(commit_plan(&plan).is_err());assert_eq!(fs::read(&a).unwrap(),b"new");let _=fs::remove_dir_all(&d);
    }
}
