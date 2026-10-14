use iced_x86::{Decoder, DecoderOptions, Formatter, Instruction, IntelFormatter, MemorySizeOptions};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::{BTreeMap, BTreeSet}, fs::{self, OpenOptions}, io::Write, path::{Component, Path, PathBuf}};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub const EXE_SHA256: &str = "6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf";
pub const WIDTHS: [u64; 16] = [4,4,4,4,4,4,4,4,4,2,4,4,4,2,2,2];
pub fn contract() -> Value {
    serde_json::from_str(include_str!("contract.json").trim_start_matches('\u{feff}')).expect("frozen contract JSON")
}
fn need(ok: bool, message: impl Into<String>) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into().into()) }
}
pub fn sha256(raw: &[u8]) -> String { format!("{:x}", Sha256::digest(raw)) }
fn hex(raw: &[u8]) -> String { raw.iter().map(|b| format!("{b:02x}")).collect() }
fn unhex(s: &str) -> Result<Vec<u8>> {
    need(!s.is_empty() && s.len() % 2 == 0 && s.is_ascii(), "invalid instruction hex")?;
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i+2],16).map_err(Into::into)).collect()
}
fn addr(s: &str) -> Result<u64> { Ok(u64::from_str_radix(s.strip_prefix("0x").ok_or("address needs 0x")?,16)?) }
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str> { v[key].as_str().ok_or_else(|| format!("missing string {key}").into()) }
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a Vec<Value>> { v[key].as_array().ok_or_else(|| format!("missing array {key}").into()) }

// Canonical Intel operands retain memory widths, register identities, ordering,
// signs and values. Only formatting aliases and implicit x87 ST0 are collapsed.
pub fn canonical(mnemonic: &str, operands: &str) -> Result<(String, String)> {
    let normalized_width = operands.replace("xword ptr", "tbyte ptr");
    let operands = normalized_width.as_str();
    let mnemonic = match mnemonic { "jz" => "je", "jnz" => "jne", "jpe" => "jp", "jpo" => "jnp", "fwait" => "wait", "sal" => "shl", "retn" => "ret", x => x }.to_owned();
    let mut out = String::new();
    let bytes = operands.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() { i += 1; continue; }
        if bytes[i].is_ascii_digit() {
            let start = i;
            let radix = if bytes[i..].starts_with(b"0x") { i += 2; 16 } else { 10 };
            let digits = i;
            while i < bytes.len() && (if radix == 16 { bytes[i].is_ascii_hexdigit() } else { bytes[i].is_ascii_digit() }) { i += 1; }
            need(i > digits, "malformed operand number")?;
            // Decimal scanner already includes its first digit.
            let text = if radix == 16 { &operands[digits..i] } else { &operands[start..i] };
            out.push_str(&u64::from_str_radix(text, radix)?.to_string());
        } else { out.push(bytes[i] as char); i += 1; }
    }
    let mut parts: Vec<String> = out.split(',').map(|x| if x == "st" { "st(0)".to_string() } else { x.to_string() }).collect();
    if parts.len() == 2 && ["fadd","fsub","fmul","fdiv","fcomp","fxch"].contains(&mnemonic.as_str()) && parts[0] == "st(0)" {
        parts.remove(0);
    }
    if parts.len() == 2 && ["faddp","fsubp","fsubrp","fmulp","fdivp","fdivrp"].contains(&mnemonic.as_str()) && parts[1] == "st(0)" {
        parts.pop();
    }
    Ok((mnemonic, parts.join(",")))
}
fn format_instruction(ins: &Instruction) -> (String, String) {
    let mut formatter = IntelFormatter::new();
    let options = formatter.options_mut();
    options.set_hex_prefix("0x"); options.set_hex_suffix("");
    options.set_uppercase_hex(false); options.set_small_hex_numbers_in_decimal(true);
    options.set_branch_leading_zeros(false); options.set_displacement_leading_zeros(false);
    options.set_add_leading_zero_to_hex_numbers(false); options.set_show_branch_size(false);
    options.set_signed_immediate_operands(false);
    options.set_use_pseudo_ops(false);
    options.set_memory_size_options(MemorySizeOptions::Always);
    options.set_space_after_operand_separator(true); options.set_prefer_st0(false);
    let mut mnemonic = String::new();
    formatter.format_mnemonic(ins, &mut mnemonic);
    let mut operands = Vec::new();
    for index in 0..formatter.operand_count(ins) {
        // iced adds implicit x87 ST0 operands that have no encoded operand.
        // Keep every actual instruction operand, including explicit ST0/STi.
        if matches!(formatter.get_instruction_operand(ins, index), Ok(None)) {
            continue;
        }
        let mut operand = String::new();
        formatter.format_operand(ins, &mut operand, index).expect("formatter operand index");
        operands.push(operand);
    }
    (mnemonic, operands.join(", "))
}
pub fn verify_instruction(raw: &[u8], va: u64, mnemonic: &str, operands: &str) -> Result<(String,String)> {
    let mut decoder = Decoder::with_ip(32, raw, va, DecoderOptions::NONE);
    let instruction = decoder.decode();
    need(!instruction.is_invalid() && instruction.len() == raw.len(), format!("invalid/partial instruction {va:#x}"))?;
    let actual = format_instruction(&instruction);
    need(canonical(&actual.0, &actual.1)? == canonical(mnemonic, operands)?,
        format!("instruction mismatch {va:#x}: {actual:?}, expected {mnemonic} {operands}"))?;
    Ok(actual)
}

pub struct Pe<'a> { source: &'a [u8], sections: Vec<(u64,usize,usize)> }
fn u16_at(raw: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(raw.get(offset..offset.checked_add(2).ok_or("overflow")?).ok_or("truncated PE")?.try_into()?))
}
fn u32_at(raw: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(raw.get(offset..offset.checked_add(4).ok_or("overflow")?).ok_or("truncated PE")?.try_into()?))
}
impl<'a> Pe<'a> {
    pub fn parse(source: &'a [u8]) -> Result<Self> {
        need(source.get(..2) == Some(b"MZ"), "missing DOS signature")?;
        let pe = u32_at(source,0x3c)? as usize;
        need(source.get(pe..pe.checked_add(4).ok_or("overflow")?) == Some(b"PE\0\0"), "missing PE signature")?;
        need(u16_at(source,pe+4)? == 0x14c, "unexpected PE architecture")?;
        let count = u16_at(source,pe+6)? as usize;
        let optional_size = u16_at(source,pe+20)? as usize;
        need(optional_size >= 96 && count > 0 && count <= 96, "invalid PE headers")?;
        need(u16_at(source,pe+24)? == 0x10b && u32_at(source,pe+52)? == 0x400000, "unexpected PE32/base")?;
        let table = pe+24+optional_size;
        let mut sections = Vec::new();
        for i in 0..count {
            let offset = table+i*40;
            let rva = u32_at(source,offset+12)? as u64;
            let size = u32_at(source,offset+16)? as usize;
            let file = u32_at(source,offset+20)? as usize;
            need(file.checked_add(size).is_some_and(|end| end <= source.len()), "truncated section")?;
            sections.push((rva,file,size));
        }
        Ok(Self { source, sections })
    }
    pub fn read(&self, va: u64, size: usize) -> Result<&'a [u8]> {
        let rva = va.checked_sub(0x400000).ok_or("VA below image base")?;
        let end = rva.checked_add(size as u64).ok_or("VA overflow")?;
        let matching: Vec<_> = self.sections.iter().filter(|(start,_,length)| rva >= *start && end <= *start+*length as u64).collect();
        need(matching.len() == 1, format!("unmapped/ambiguous/truncated PE region {va:#x}"))?;
        let (start,file,_) = *matching[0];
        let offset = file+(rva-start) as usize;
        self.source.get(offset..offset+size).ok_or_else(|| "truncated PE region".into())
    }
}

pub fn cross_check(reference: &Value) -> Result<()> {
    let frozen = contract();
    for key in ["exe_sha256","instruction_assertions","assertions","fields","render_radius_source","limitations","reused_evidence"] {
        need(reference[key] == frozen[key], format!("strengthened reference disagreement: {key}"))?;
    }
    let regions = array(reference,"regions")?;
    let expected = array(&frozen,"regions")?;
    need(regions.len() == expected.len(), "reference region count mismatch")?;
    for (r,e) in regions.iter().zip(expected) {
        for key in ["start","size","sha256","instructions"] {
            need(r[key] == e[key], format!("reference region disagreement: {key}"))?;
        }
    }
    Ok(())
}
pub fn validate_fields(data: &Value) -> Result<()> {
    let fields = array(data,"fields")?;
    need(fields.len() == 16, "field count mismatch")?;
    let offsets = [0u64,4,8,12,16,20,24,28,32,36,40,44,48,52,54,56];
    let names = ["active_raw","x","y","z","red","green","blue","effective_radius","baseline_radius","type_raw","type_parameter","age_raw","special_fx_raw","owner","parent","definition"];
    let strides = [0x48ac02u64,0x48ac50,0x48aca0,0x48acf2,0x48ad44,0x48ad94,0x48ade6,0x48ae38,0x48ae88,0x48aece,0x48af1d,0x48af61,0x48afa3,0x48afe5,0x48b024,0x48b05f];
    let assertions = array(data,"assertions")?;
    let find = |va: u64| -> Result<&Value> {
        assertions.iter().find(|a| a["address"].as_str().and_then(|s| addr(s).ok()) == Some(va)).ok_or_else(|| format!("missing mapping instruction {va:#x}").into())
    };
    let mut packed = 0;
    for (column,field) in fields.iter().enumerate() {
        let width = WIDTHS[column];
        need(field["column"] == column && field["width"] == width && field["name"] == names[column] && field["snapshot_offset"] == packed && addr(string(field,"runtime_offset")?)? == offsets[column], "field mapping mismatch")?;
        need(field["pool_offset_formula"] == format!("8 + capacity * {packed} + slot * {width}"), "pool offset formula mismatch")?;
        let read = find(addr(string(field,"read_va")?)?)?;
        let target = if width == 2 { "0x414e40" } else if [0,11,12].contains(&column) { "0x414e90" } else { "0x414f10" };
        need(canonical(string(read,"mnemonic")?,string(read,"operands")?)? == canonical("call",target)?, "reader mapping mismatch")?;
        let store = find(addr(string(field,"store_va")?)?)?;
        let operands = format!("{} ptr [edi - {:#x}], {}",if width == 2 { "word" } else { "dword" },0x44-offsets[column],if width == 2 { "ax" } else { "eax" });
        need(canonical(string(store,"mnemonic")?,string(store,"operands")?)? == canonical("mov",&operands)?, "store mapping mismatch")?;
        let stride = find(strides[column])?;
        need(canonical(string(stride,"mnemonic")?,string(stride,"operands")?)? == canonical("add","edi, 0x44")?, "runtime stride mismatch")?;
        packed += width;
    }
    need(packed == 56 && 8+1024*packed == 57352, "saved layout mismatch")
}
pub fn inspect(source: &[u8]) -> Result<Value> {
    need(sha256(source) == EXE_SHA256, "unsupported EXE SHA256")?;
    let pe = Pe::parse(source)?;
    inspect_regions(&pe)
}
fn inspect_regions(pe: &Pe<'_>) -> Result<Value> {
    let mut result = contract();
    validate_fields(&result)?;
    let mut decoded = BTreeMap::new();
    for region in result["regions"].as_array_mut().ok_or("missing regions")? {
        let start = addr(string(region,"start")?)?;
        let size = region["size"].as_u64().ok_or("missing region size")? as usize;
        let raw = pe.read(start,size)?;
        need(sha256(raw) == string(region,"sha256")?, "region hash mismatch")?;
        let mut decoder = Decoder::with_ip(32,raw,start,DecoderOptions::NONE);
        let mut lines = Vec::new();
        while decoder.can_decode() {
            let ins = decoder.decode();
            need(!ins.is_invalid(), format!("invalid decode at {:#x}",ins.ip()))?;
            let offset = (ins.ip()-start) as usize;
            let bytes = raw.get(offset..offset+ins.len()).ok_or("decode overran region")?;
            let (mnemonic, operands) = format_instruction(&ins);
            lines.push(format!("{:#x} {} {} {}",ins.ip(),hex(bytes),mnemonic,operands));
            need(decoded.insert(ins.ip(),hex(bytes)).is_none(), "overlapping instruction regions")?;
        }
        need(decoder.position() == size, "incomplete region decode")?;
        region["instructions"] = json!(lines);
    }
    let assertions = result["assertions"].as_array_mut().ok_or("missing assertions")?;
    need(assertions.len() == 746, "frozen assertion count mismatch")?;
    let mut seen = BTreeSet::new();
    for assertion in assertions {
        let va = addr(string(assertion,"address")?)?;
        let expected = unhex(string(assertion,"bytes")?)?;
        need(seen.insert(va), "duplicate assertion address")?;
        let raw = pe.read(va,expected.len())?;
        need(raw == expected && decoded.get(&va) == Some(&hex(raw)), format!("assertion bytes/boundary mismatch {va:#x}"))?;
        let (mnemonic,operands) = verify_instruction(raw,va,string(assertion,"mnemonic")?,string(assertion,"operands")?)?;
        assertion["native_mnemonic"] = json!(mnemonic);
        assertion["native_operands"] = json!(operands);
    }
    Ok(result)
}

fn instruction_line(line: &str) -> Result<(u64,Vec<u8>,String,String)> {
    let mut parts = line.splitn(4,' ');
    let va = addr(parts.next().ok_or("missing address")?)?;
    let bytes = unhex(parts.next().ok_or("missing instruction bytes")?)?;
    let mnemonic = parts.next().filter(|x| !x.is_empty()).ok_or("missing mnemonic")?.to_string();
    let operands = parts.next().unwrap_or("").to_string();
    verify_instruction(&bytes,va,&mnemonic,&operands)?;
    Ok((va,bytes,mnemonic,operands))
}
pub fn validate_evidence(data: &Value, kind: &str) -> Result<Value> {
    need(data.is_object(), "evidence root must be object")?;
    if let Some(hash) = data.get("exe_sha256") { need(hash == EXE_SHA256, "evidence EXE hash mismatch")?; }
    if kind == "static" || kind == "transform" {
        need(data["exe_sha256"] == EXE_SHA256, "missing/mismatched evidence exe_sha256")?;
        let lines = array(data,"instructions")?;
        need(!lines.is_empty(), "empty evidence instructions")?;
        let count = data["instruction_assertions"].as_u64().ok_or("missing positive instruction_assertions")?;
        need(count > 0 && count <= lines.len() as u64, "invalid assertion count")?;
        let mut seen = BTreeSet::new();
        for line in lines {
            let (va,_,_,_) = instruction_line(line.as_str().ok_or("instruction line must be string")?)?;
            need(seen.insert(va), "duplicate evidence instruction address")?;
        }
        if kind == "static" { need(data["light_column_widths"] == json!(WIDTHS), "column widths mismatch")?; }
        else {
            let xyz = data["symbolic_xyz"].as_object().ok_or("missing symbolic_xyz")?;
            let keys: BTreeSet<_> = xyz.keys().map(String::as_str).collect();
            need(keys == BTreeSet::from(["dword ptr [esp + 0x20]","dword ptr [esp + 4]","dword ptr [esp]"]), "symbolic_xyz keys mismatch")?;
            need(data["callback_fields"] == json!([67,70,71]) && data["owner_position_runtime_offset"] == 12, "transform metadata mismatch")?;
        }
        return Ok(json!({"instruction_assertions":count,"source_binding":"exe_sha256"}));
    }
    need(kind == "bundle", "unsupported evidence schema kind")?;
    let records = array(data,"records")?;
    need(!records.is_empty(), "empty evidence records")?;
    let mut summaries = Vec::new();
    let mut ids = BTreeSet::new();
    for record in records {
        need(record.is_object() && record["subject"]["digest"]["sha256"] == EXE_SHA256, "missing/mismatched record subject digest")?;
        let id = string(record,"evidence_id")?;
        let operation = string(record,"operation")?;
        need(!id.is_empty() && !operation.is_empty() && ids.insert(id), "empty/duplicate evidence identity")?;
        need(record["parameters"].is_object(), "missing parameters object")?;
        summaries.push(json!({"id":id,"operation":operation,"parameters":record["parameters"]}));
    }
    Ok(json!({"records":summaries,"source_binding":"every record subject.digest.sha256"}))
}

// Canonical parent resolves Windows junctions/symlinks. File creation remains
// create_new, rejecting a target appearing between validation and opening.
pub fn output_path(repo: &Path, output: &Path) -> Result<PathBuf> {
    need(output.extension().and_then(|x| x.to_str()) == Some("json"), "output must be JSON")?;
    let name = output.file_name().and_then(|x| x.to_str()).ok_or("missing output filename")?;
    need(!name.contains(':') && !name.ends_with('.') && !name.ends_with(' '), "invalid Windows filename/ADS")?;
    let stem = name.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    need(!["CON","PRN","AUX","NUL","CONIN$","CONOUT$"].contains(&stem.as_str()) &&
        !(stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && matches!(stem.as_bytes()[3], b'1'..=b'9')),
        "Windows device output forbidden")?;
    need(!output.components().any(|c| c == Component::ParentDir), "output parent traversal forbidden")?;
    let parent = output.parent().filter(|x| !x.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent)?;
    need(parent == fs::canonicalize(repo.join("re_workspace"))?, "output must be directly in repository re_workspace")?;
    let target = parent.join(name);
    need(fs::symlink_metadata(&target).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound), "refusing existing/inaccessible output")?;
    Ok(target)
}
pub fn run(repo: &Path, exe: &Path, output: &Path, reference: &Path) -> Result<Value> {
    let target = output_path(repo,output)?;
    let reference_raw = fs::read(reference)?;
    let reference_data: Value = serde_json::from_slice(&reference_raw)?;
    cross_check(&reference_data)?;
    let source = fs::read(exe)?;
    let pe = Pe::parse(&source)?;
    let mut result = inspect(&source)?;
    let frozen = contract();
    let mut reused = Vec::new();
    for evidence in array(&frozen,"reused_evidence")? {
        let name = string(evidence,"path")?.replace('\\',"/");
        let path = repo.join(&name);
        let raw = fs::read(&path)?;
        let data: Value = serde_json::from_slice(&raw)?;
        let kind = string(evidence,"schema")?;
        let mut summary = validate_evidence(&data,kind)?;
        need(sha256(&raw) == string(evidence,"sha256")?, format!("frozen evidence hash disagreement: {name}"))?;
        if kind == "static" || kind == "transform" {
            let mut owner = BTreeSet::new();
            let mut matched = Vec::new();
            for line in array(&data,"instructions")? {
                let (va,bytes,mnemonic,operands) = instruction_line(line.as_str().ok_or("malformed evidence line")?)?;
                need(pe.read(va,bytes.len())? == bytes, format!("evidence instruction bytes mismatch {va:#x}"))?;
                verify_instruction(&bytes,va,&mnemonic,&operands)?;
                if kind == "transform" && (0x49f8fa..=0x49f9d7).contains(&va) {
                    let a = array(&frozen,"assertions")?.iter().find(|a| addr(a["address"].as_str().unwrap_or("")).ok() == Some(va)).ok_or("unknown owner assertion")?;
                    need(a["bytes"] == hex(&bytes) && canonical(string(a,"mnemonic")?,string(a,"operands")?)? == canonical(&mnemonic,&operands)?, "owner instruction disagreement")?;
                    owner.insert(va); matched.push(format!("{va:#x}"));
                }
            }
            if kind == "transform" {
                let required: BTreeSet<_> = array(&frozen,"assertions")?.iter().map(|a| addr(a["address"].as_str().unwrap_or(""))).collect::<Result<Vec<_>>>()?.into_iter().filter(|va| (0x49f8fa..=0x49f9d7).contains(va)).collect();
                need(owner == required, "incomplete owner transform coverage")?;
                summary["matched_owner_instructions"] = json!(matched);
                summary["symbolic_xyz"] = data["symbolic_xyz"].clone();
            }
        }
        need(fs::read(&path)? == raw, "evidence changed during proof")?;
        summary["path"] = json!(name); summary["sha256"] = json!(sha256(&raw)); summary["schema"] = json!(kind);
        reused.push(summary);
    }
    need(fs::read(exe)? == source && fs::read(reference)? == reference_raw, "EXE/reference changed during proof")?;
    result["reused_evidence"] = json!(reused);
    result["source_unchanged"] = json!(true);
    result["reference_cross_check"] = json!({"path":reference,"sha256":sha256(&reference_raw),"assertions":746,"fields":16,"regions":9});
    let binary = fs::read(std::env::current_exe()?)?;
    result["probe_sha256"] = json!(sha256(&binary));
    result["tool_versions"] = json!({"implementation":"Rust native","crate":env!("CARGO_PKG_VERSION"),"iced-x86":"1.21.0"});
    let mut serialized = serde_json::to_vec_pretty(&result)?; serialized.push(b'\n');
    let mut stream = OpenOptions::new().write(true).create_new(true).open(&target)?;
    stream.write_all(&serialized)?; stream.sync_all()?;
    Ok(json!({"output":target,"instruction_assertions":746,"exe_sha256":EXE_SHA256,"source_unchanged":true}))
}
