//! Backup, discovery and patch composition services. Planning never writes files.
use std::{collections::{BTreeMap, HashMap}, fs, io::{self, Read, Write}, path::{Path, PathBuf}};
use crate::*;

pub const DEFAULT_INSTALL_PATH: &str = r"C:\Program Files (x86)\Against Rome";
pub const OBJDEF_KEY: &str = "SYSTEM/DATA_MP/DEFAULTS/objdef.dau";
const BASELINE: &[&str] = &["Against_Rome.exe", "SYSTEM/cl_script.ini", "SYSTEM/cl_epara.ini", "SYSTEM/ress.ini", OBJDEF_KEY, "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau", "SYSTEM/CLMK/icon.ini", "SYSTEM/CLAK/cl_scint.ini", "SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci"];
fn error(s: impl Into<String>) -> io::Error { io::Error::new(io::ErrorKind::InvalidData,s.into()) }
fn normalized(key:&str)->String { key.replace('\\',"/").to_lowercase() }
fn safe_key(key:&str)->io::Result<String> {
    let key=key.replace('\\',"/");
    if key.starts_with('/') || key.as_bytes().get(1)==Some(&b':') || key.split('/').any(|p|p=="..") || key.contains('\0') { return Err(error(format!("Backup.zip contains an unsafe entry path: {key}"))); }
    Ok(key)
}
fn resolve(root:&Path,key:&str)->io::Result<PathBuf>{ Ok(root.join(safe_key(key)?)) }
fn team_key(key:&str)->bool { let k=normalized(key); k.starts_with("maps/") && k.ends_with("team.dat") }
pub fn is_custom_map_file(maps:&Path,file:&Path)->bool { file.strip_prefix(maps).ok().and_then(|r|r.components().next()).is_some_and(|c| maps.join(c).join(".arm_custom_map").is_file()) }
fn team_files(root:&Path)->io::Result<Vec<PathBuf>> {
    fn visit(path:&Path,output:&mut Vec<PathBuf>)->io::Result<()> { if !path.is_dir(){return Ok(())} for e in fs::read_dir(path)? { let e=e?; if e.file_type()?.is_symlink(){continue} if e.file_type()?.is_dir(){visit(&e.path(),output)?} else if e.file_name().to_string_lossy().eq_ignore_ascii_case("team.dat"){output.push(e.path())} } Ok(()) }
    let mut output=Vec::new();visit(&root.join("MAPS"),&mut output)?;output.retain(|p|!is_custom_map_file(&root.join("MAPS"),p));output.sort();Ok(output)
}

pub struct GameDirectoryLocator;
impl GameDirectoryLocator {
    pub fn resolve_initial_game_path(preferred:Option<&Path>)->PathBuf {
        let base=std::env::current_exe().ok().and_then(|p|p.parent().map(Path::to_path_buf)).unwrap_or_default();
        Self::resolve_with(preferred,&base,&Self::detect_from_registry(),Path::new(DEFAULT_INSTALL_PATH))
    }
    pub fn resolve_with(preferred:Option<&Path>,base:&Path,registry:&Path,default:&Path)->PathBuf {
        if let Some(p)=preferred.filter(|p|!p.as_os_str().is_empty()&&p.is_dir()){return p.to_owned()}
        if base.join("Against_Rome.exe").is_file(){return base.to_owned()}
        if !registry.as_os_str().is_empty(){return registry.to_owned()}
        if default.is_dir(){return default.to_owned()} PathBuf::new()
    }
    #[cfg(not(windows))]
    pub fn detect_from_registry()->PathBuf {PathBuf::new()}
    #[cfg(windows)]
    pub fn detect_from_registry()->PathBuf {
        use std::os::windows::ffi::OsStringExt;
        #[link(name="advapi32")] extern "system" {fn RegGetValueW(key:isize,sub:*const u16,value:*const u16,flags:u32,kind:*mut u32,data:*mut core::ffi::c_void,size:*mut u32)->i32;}
        for (root,sub) in [(0x80000001u32 as i32 as isize,r"Software\Against Rome"),(0x80000002u32 as i32 as isize,r"SOFTWARE\Against Rome"),(0x80000002u32 as i32 as isize,r"SOFTWARE\WOW6432Node\Against Rome")] {
            let sub:Vec<u16>=sub.encode_utf16().chain(Some(0)).collect();let value:Vec<u16>="Path".encode_utf16().chain(Some(0)).collect();let mut size=0;
            if unsafe{RegGetValueW(root,sub.as_ptr(),value.as_ptr(),6,std::ptr::null_mut(),std::ptr::null_mut(),&mut size)}!=0 {continue}
            let mut data=vec![0u16;(size as usize+1)/2];
            if unsafe{RegGetValueW(root,sub.as_ptr(),value.as_ptr(),6,std::ptr::null_mut(),data.as_mut_ptr().cast(),&mut size)}==0 {while data.last()==Some(&0){data.pop();} return PathBuf::from(std::ffi::OsString::from_wide(&data));}
        } PathBuf::new()
    }
}

pub struct OriginalFileValidator;
impl OriginalFileValidator {
    pub fn is_exe_original(bytes:&[u8])->bool {
        // The complete EXE validator is supplied by the executable feature model.
        crate::detectors::is_exe_original(bytes)
    }
    pub fn is_objdef_original(bytes:&[u8])->bool {
        let Ok(raw)=decompress_pfil(bytes) else{return false};let text=decode_cp1251(&raw);
        for line in text.lines().filter(|l|l.len()>=100) {let cols=parse_csv_line(line);if cols.len()>=192&&cols[52].trim()=="FigRomAnf00_Anfuehrer" {return cols[19].trim().parse::<f64>().is_ok_and(|hp|(hp-400.0).abs()<=0.1)}} false
    }
    pub fn is_partgeo_original(bytes:&[u8])->bool {
        let Ok(raw)=decompress_pfil(bytes) else{return false};let text=decode_cp1251(&raw);
        for line in text.lines(){let c=parse_csv_line(line);if c.len()>partgeo::YSUB_COLUMN&&c[2].trim()=="Pfeil00" {return c[partgeo::YSUB_COLUMN].trim().parse::<i64>()==Ok(5832704)}} false
    }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum BackupZipSource {None,Embedded,Local,GameDirectory}
#[derive(Debug,Default,Clone)]
pub struct BackupManager { files:BTreeMap<String,Vec<u8>>, pub logs:Vec<String> }
impl BackupManager {
    pub fn new()->Self {Self::default()}
    pub fn backup_files(&self)->&BTreeMap<String,Vec<u8>> {&self.files}
    pub fn has_file(&self,key:&str)->bool {self.files.contains_key(&normalized(key))}
    pub fn get_backup_bytes(&self,key:&str)->io::Result<&[u8]> {self.files.get(&normalized(key)).map(Vec::as_slice).ok_or_else(||error(format!("記憶體備份中找不到 {key}。")))}
    pub fn set_backup_file(&mut self,key:&str,bytes:Vec<u8>) {self.files.insert(normalized(key),bytes);}
    pub fn clear(&mut self){self.files.clear()}
    pub fn find_missing_backup_resources(&self)->Vec<String> {let mut missing:Vec<_>=BASELINE.iter().filter(|k|!k.ends_with("partgeo.dau")&&!self.has_file(k)).map(|s|s.to_string()).collect();if !self.files.keys().any(|k|team_key(k)){missing.push("MAPS/.../team.dat".into())}missing}
    pub fn validate_backup_resources(&self)->io::Result<()> {let m=self.find_missing_backup_resources();if m.is_empty(){Ok(())}else{Err(error(format!("備份資源不完整\r\n{}",m.join("\r\n"))))}}
    pub fn load_zip(bytes:&[u8])->io::Result<BTreeMap<String,Vec<u8>>> {
        let mut archive=zip::ZipArchive::new(io::Cursor::new(bytes)).map_err(|e|error(e.to_string()))?;let mut files=BTreeMap::new();
        for i in 0..archive.len(){let mut e=archive.by_index(i).map_err(|e|error(e.to_string()))?;if e.name().ends_with('/') {continue}let key=safe_key(e.name())?;let mut bytes=Vec::new();e.read_to_end(&mut bytes)?;files.insert(normalized(&key),bytes);} Ok(files)
    }
    pub fn load_backup_zip_to_memory(&mut self,game:&Path,embedded:Option<&[u8]>,base:&Path)->io::Result<BackupZipSource> {
        let (source,data)=if let Some(bytes)=embedded {(BackupZipSource::Embedded,Some(bytes.to_vec()))}else if base.join("Backup.zip").is_file(){(BackupZipSource::Local,Some(fs::read(base.join("Backup.zip"))?))}else{(BackupZipSource::None,None)};
        if let Some(bytes)=data {self.files=Self::load_zip(&bytes)?;self.try_auto_heal_backup_files(game);self.validate_backup_resources()?;return Ok(source)}
        if self.try_load_backup_from_game_directory(game,false)?{Ok(BackupZipSource::GameDirectory)}else{Ok(BackupZipSource::None)}
    }
    pub fn ensure_backup_loaded_for_game_path(&mut self,game:&Path)->io::Result<bool>{if !self.files.is_empty()&&self.find_missing_backup_resources().is_empty(){return Ok(true)}self.try_load_backup_from_game_directory(game,true)}
    pub fn try_load_backup_from_game_directory(&mut self,game:&Path,show_error:bool)->io::Result<bool> {
        if game.as_os_str().is_empty()||!game.is_dir(){return if show_error{Err(error("Backup.zip not found and game path is invalid."))}else{Ok(false)}}
        let load=||->io::Result<BTreeMap<String,Vec<u8>>>{let mut files=BTreeMap::new();
            for &key in BASELINE {if key=="SYSTEM/cl_epara.ini"{files.insert(normalized(key),CleanEparaBaseline::create_bytes()?);continue}let path=resolve(game,key)?;let bak=PathBuf::from(format!("{}.bak",path.display()));if bak.is_file(){files.insert(normalized(key),fs::read(bak)?);continue}if !path.is_file(){continue}let bytes=fs::read(&path)?;let original=match key{"Against_Rome.exe"=>OriginalFileValidator::is_exe_original(&bytes),OBJDEF_KEY=>OriginalFileValidator::is_objdef_original(&bytes),"SYSTEM/DATA_MP/DEFAULTS/partgeo.dau"=>OriginalFileValidator::is_partgeo_original(&bytes),_=>true};if !original{return Err(error(format!("{key} is already modified or not supported. Please restore original file first.")))}copy_exclusive(&bak,&bytes)?;files.insert(normalized(key),bytes);}
            for path in team_files(game)? {let bak=PathBuf::from(format!("{}.bak",path.display()));let bytes=if bak.is_file(){fs::read(&bak)?}else{let bytes=fs::read(&path)?;copy_exclusive(&bak,&bytes)?;bytes};files.insert(normalized(&path.strip_prefix(game).map_err(|e|error(e.to_string()))?.to_string_lossy()),bytes);}Ok(files)};
        match load(){Ok(files)=>self.files=files,Err(e)=>{if show_error{return Err(e)}self.logs.push(e.to_string());return Ok(false)}}
        self.try_auto_heal_backup_files(game);let m=self.find_missing_backup_resources();if !m.is_empty(){return if show_error{Err(error(format!("遊戲目錄中缺少必要檔案，無法建立完整備份:\r\n{}",m.join("\r\n"))))}else{Ok(false)}}Ok(true)
    }
    pub fn try_auto_heal_backup_files(&mut self,game:&Path)->bool {
        if !game.is_dir(){return false}let mut changed=false;
        for &key in BASELINE {if self.has_file(key){continue}let heal=||->io::Result<Option<Vec<u8>>>{if key=="SYSTEM/cl_epara.ini"{return Ok(Some(CleanEparaBaseline::create_bytes()?))}let path=resolve(game,key)?;let bak=PathBuf::from(format!("{}.bak",path.display()));let source=if bak.is_file(){&bak}else{&path};if !source.is_file(){return Ok(None)}let bytes=fs::read(source)?;if key.ends_with("partgeo.dau"){if !OriginalFileValidator::is_partgeo_original(&bytes){return Err(error("partgeo.dau is already modified"))}if !bak.is_file(){copy_exclusive(&bak,&bytes)?}}Ok(Some(bytes))};match heal(){Ok(Some(bytes))=>{self.set_backup_file(key,bytes);changed=true},Err(e)=>self.logs.push(e.to_string()),_=>()}}
        if !self.files.keys().any(|k|team_key(k)){match team_files(game){Ok(paths)=>for p in paths {let bak=PathBuf::from(format!("{}.bak",p.display()));let result=fs::read(if bak.is_file(){&bak}else{&p});match result{Ok(bytes)=>{if let Ok(rel)=p.strip_prefix(game){self.set_backup_file(&rel.to_string_lossy(),bytes);changed=true}},Err(e)=>self.logs.push(e.to_string())}},Err(e)=>self.logs.push(e.to_string())}}changed
    }
    pub fn get_backup_unit_rows(&self)->io::Result<HashMap<String,Vec<String>>> {
        let mut rows=HashMap::new();if !self.has_file(OBJDEF_KEY){return Ok(rows)}let doc=TextDocument::read(self.get_backup_bytes(OBJDEF_KEY)?)?;
        for line in doc.lines.iter().skip(2).filter(|l|l.len()>=100){let c=parse_csv_line(line);if c.len()<192{continue}let name=c[52].trim().to_owned();if TroopConfig::get_metadata(&name).is_some()||name=="FigZivMan00_Zivilist"{rows.insert(name,c);}}Ok(rows)
    }
    pub fn get_original_stats(&self,key:&str)->io::Result<[f64;9]> {
        let rows=self.get_backup_unit_rows()?;let Some((_,c))=rows.iter().find(|(k,_)|k.eq_ignore_ascii_case(key))else{return Ok([0.;9])};let unit=TroopConfig::get_metadata(key).map(|m|m.unit_type).unwrap_or("melee_inf");let (md,rd)=UnitStatParser::get_melee_and_ranged_damage(c,unit);let (mr,rr)=UnitStatParser::get_melee_and_ranged_reload(c,unit);let (d,r)=match unit{"ranged_inf"|"ranged_cav"=>(rd,rr),"siege"=>(md.max(rd),mr.max(rr)),_=>(md,mr)};let n=|i:usize|c[i].trim().parse::<f64>().unwrap_or(0.);let speed=n(4);Ok([n(19),d,n(146),n(142),if speed>0.{(speed*20.).round_ties_even()/10.}else{0.},n(24),r,UnitStatParser::get_maximum_range(c,unit),if TroopConfig::supports_configurable_spell_radius(key){500.}else{0.}])
    }
    pub fn get_default_balanced_stats(&self,key:&str)->io::Result<[f64;9]>{TroopConfig::get_balanced_stats(key).copied().map(Ok).unwrap_or_else(||self.get_original_stats(key))}
    pub fn get_base_stats_for_unit(&self,key:&str,profile:&PatchProfile)->io::Result<[f64;9]> {
        let baseline=if profile.get_bool("Balance"){self.get_default_balanced_stats(key)?}else{return self.get_original_stats(key)};
        let custom=match profile.get("CustomUnitStats"){FeatureValue::String(s) if !s.is_empty()=>serde_json::from_str::<BTreeMap<String,Vec<f64>>>(&s).map_err(|e|error(e.to_string()))?,_=>BTreeMap::new()};
        if let Some((_,values))=custom.iter().find(|(k,_)|k.eq_ignore_ascii_case(key)){return Ok(UnitStatParser::merge_layers(&baseline,values,TroopConfig::supports_configurable_spell_radius(key),TroopConfig::get_metadata(key).is_some_and(|m|m.unit_type=="priest")))}Ok(baseline)
    }
}
fn copy_exclusive(path:&Path,bytes:&[u8])->io::Result<()> {let mut f=fs::OpenOptions::new().create_new(true).write(true).open(path)?;f.write_all(bytes)?;f.sync_all()}

#[derive(Debug,Default,Clone)]
pub struct PatchFilePlan {pub files:BTreeMap<PathBuf,Vec<u8>>,pub deletes:Vec<PathBuf>}
pub struct PatchEngine;
impl PatchEngine {
    pub fn compose(game:&Path,p:&PatchProfile,b:&BackupManager)->io::Result<PatchFilePlan> {
        let mut output=PatchFilePlan::default();let mut add=|key:&str,bytes:Vec<u8>|->io::Result<()>{output.files.insert(resolve(game,key)?,bytes);Ok(())};
        let exe=fs::read(game.join("Against_Rome.exe"))?;
        let patched=crate::exe_features::apply_profile(&exe,p)?;if patched!=exe{add("Against_Rome.exe",patched)?}
        let balanced=|key:&str|->io::Result<f64>{if p.get_bool("Balance"){Ok(b.get_default_balanced_stats(key)?[8]/500.)}else{Ok(1.)}};
        add("SYSTEM/cl_script.ini",ClScriptPatcher::get_patched_bytes(b.get_backup_bytes("SYSTEM/cl_script.ini")?,&ClScriptOptions {fast_civilian_production:p.get_bool("FastCiviProduction"),infinite_morale:p.get_bool("InfiniteMorale"),spell_damage_5x:p.get_bool("SpellDamage5x"),spell_healing_10x:p.get_bool("SpellHealing10x"),spell_resurrection:p.get_bool("SpellResurrection"),general_skills_5x:p.get_bool("GeneralSkills"),german_spell_radius_multiplier:balanced("FigGerPri00_Priester")?,celt_spell_radius_multiplier:balanced("FigKelPri00_Priester")?,hun_spell_radius_multiplier:balanced("FigHunPri00_Priester")?,spell_range_3x:p.get_bool("SpellRange3x"),original_bytes:None})?)?;
        add("SYSTEM/cl_epara.ini",EparaPatcher::get_patched_bytes(b.get_backup_bytes("SYSTEM/cl_epara.ini")?,p.get_bool("RangedRange3x"))?)?;
        let part="SYSTEM/DATA_MP/DEFAULTS/partgeo.dau";if b.has_file(part){add(part,PartgeoPatcher::get_patched_bytes(b.get_backup_bytes(part)?,&PartgeoOptions{projectile_arc_height:p.get_bool("ProjectileArcHeight")})?)?}else if p.get_bool("ProjectileArcHeight"){return Err(error("partgeo.dau backup missing; cannot apply projectile arc height."))}
        add("SYSTEM/CLAK/cl_scint.ini",b.get_backup_bytes("SYSTEM/CLAK/cl_scint.ini")?.to_vec())?;
        add("SYSTEM/ress.ini",RessPatcher::get_patched_bytes(b.get_backup_bytes("SYSTEM/ress.ini")?,&RessOptions{free_production:p.get_bool("FreeProduction"),free_upgrades:p.get_bool("FreeUpgrade"),no_spell_cost:p.get_bool("NoSpellCost")})?)?;
        let mut unit_stats=HashMap::new();for key in TroopConfig::sorted_units(){unit_stats.insert(key.to_owned(),b.get_base_stats_for_unit(key,p)?);}
        add(OBJDEF_KEY,ObjdefPatcher::get_patched_bytes(b.get_backup_bytes(OBJDEF_KEY)?,&ObjdefOptions{balance:p.get_bool("Balance"),housing_capacity_20x:p.get_bool("HousingCapacity20x"),storage_capacity_10x:p.get_bool("StorageCapacity10x"),fast_build_upgrade_repair:p.get_bool("FastBuildUpgradeRepair"),hq_hp_10x:p.get_bool("HqHp10x"),leader_glory_5x:p.get_bool("LeaderGlory"),ranged_range_3x:p.get_bool("RangedRange3x"),unit_movement_speed_2x:p.get_bool("UnitMovementSpeed2x"),spell_entire_map:p.get_bool("SpellEntireMap"),spell_range_3x:p.get_bool("SpellRange3x"),projectile_arc_height:p.get_bool("ProjectileArcHeight"),unit_stats,all_units_entire_map_vision:p.get_bool("AllUnitsEntireMapVision"),villager_movement_speed_5x:p.get_bool("VillagerMovementSpeed5x"),no_run_hp_loss:p.get_bool("NoRunHpLoss")})?)?;
        for (key,bytes) in b.backup_files().iter().filter(|(k,_)|team_key(k)){if !resolve(game,key)?.is_file(){continue}add(key,TeamDatPatcher::get_patched_bytes(bytes,&TeamDatOptions{max_population:p.get_bool("MaxPopulation"),roman_player:p.get_bool("RomanEndless")&&key.to_lowercase().starts_with("maps/endl_"),..Default::default()})?)?}Ok(output)
    }

    pub fn is_dgvoodoo_installed(game_path: &Path) -> bool {
        crate::install::DgVoodooFeature::is_installed(game_path)
    }

    pub fn is_argm_trace_installed(game_path: &Path) -> bool {
        crate::install::ArgmTraceFeature::is_installed(game_path)
    }

    pub fn detect_current_patch_state(game_path: &Path, backup_manager: &BackupManager) -> PatchProfile {
        crate::detectors::FeatureDetector::detect(game_path, backup_manager)
    }

    pub fn restore_original_files(game_path: &Path, backup: &BackupManager, preserve_custom_maps: bool) -> io::Result<PatchFilePlan> {
        let mut plan = PatchFilePlan::default();
        for (key, bytes) in backup.backup_files() {
            let path = resolve(game_path, key)?;
            if preserve_custom_maps && is_custom_map_file(&game_path.join("MAPS"), &path) {
                continue;
            }
            if path.is_file() {
                plan.files.insert(path, bytes.clone());
            }
        }
        // Also restore Endless AI and food healing
        let mut orch = crate::endless_ai::EndlessAiOrchestrator::new(preserve_custom_maps);
        let _ = orch.apply_mandatory_repair(game_path);
        for m_id in &orch.user_modules.clone() {
            let _ = orch.apply_module_by_id(game_path, m_id, false);
        }
        let ak_anf = backup.backup_files().get("system/clak/script/ak_anfuehrer.bci").map(Vec::as_slice);
        let _ = crate::bci_features::FoodHealingFeature::apply(game_path, false, ak_anf, &mut orch);
        let _ = crate::bci_features::RetiredDefaultSpecialArrowsMigration::restore_if_present(game_path, &mut orch);
        let _ = crate::bci_features::CiviProduce20Feature::restore_ak_npc_original(game_path, &mut orch);
        if let Ok(bci_saves) = orch.save_all() {
            for (p, b) in bci_saves {
                plan.files.insert(p, b);
            }
        }
        // Uninstall DgVoodoo and ArgmTrace and LanguagePack
        let res = crate::install::InstallResources::default();
        if let Ok(dg) = crate::install::DgVoodooFeature::plan(game_path, false, false, &res) {
            plan.files.extend(dg.writes);
            plan.deletes.extend(dg.deletes);
        }
        if let Ok(trace) = crate::install::ArgmTraceFeature::plan(game_path, false, &res) {
            plan.files.extend(trace.writes);
            plan.deletes.extend(trace.deletes);
        }
        if let Ok(lang) = crate::install::LanguagePackFeature::plan(game_path, false) {
            plan.files.extend(lang.writes);
            plan.deletes.extend(lang.deletes);
        }
        Ok(plan)
    }

    pub fn restore_stats_only(game_path: &Path, backup: &BackupManager) -> io::Result<PatchFilePlan> {
        let mut plan = PatchFilePlan::default();
        for &key in &[OBJDEF_KEY, "SYSTEM/cl_script.ini", "SYSTEM/cl_epara.ini", "SYSTEM/ress.ini", "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau"] {
            if backup.has_file(key) {
                plan.files.insert(resolve(game_path, key)?, backup.get_backup_bytes(key)?.to_vec());
            }
        }
        for (key, bytes) in backup.backup_files().iter().filter(|(k, _)| team_key(k)) {
            let path = resolve(game_path, key)?;
            if path.is_file() {
                plan.files.insert(path, bytes.clone());
            }
        }
        Ok(plan)
    }

    pub fn restore_compat_only(game_path: &Path, backup: &BackupManager) -> io::Result<PatchFilePlan> {
        let mut plan = PatchFilePlan::default();
        if backup.has_file("Against_Rome.exe") {
            let exe_path = game_path.join("Against_Rome.exe");
            if exe_path.is_file() {
                plan.files.insert(exe_path, backup.get_backup_bytes("Against_Rome.exe")?.to_vec());
            }
        }
        let res = crate::install::InstallResources::default();
        if let Ok(dg) = crate::install::DgVoodooFeature::plan(game_path, false, false, &res) {
            plan.files.extend(dg.writes);
            plan.deletes.extend(dg.deletes);
        }
        if let Ok(trace) = crate::install::ArgmTraceFeature::plan(game_path, false, &res) {
            plan.files.extend(trace.writes);
            plan.deletes.extend(trace.deletes);
        }
        Ok(plan)
    }

    pub fn restore_language_only(game_path: &Path) -> io::Result<PatchFilePlan> {
        let mut plan = PatchFilePlan::default();
        if let Ok(lang) = crate::install::LanguagePackFeature::plan(game_path, false) {
            plan.files.extend(lang.writes);
            plan.deletes.extend(lang.deletes);
        }
        Ok(plan)
    }
}
