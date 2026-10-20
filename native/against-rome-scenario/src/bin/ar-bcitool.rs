//! Command-line port of tools/bcitool.py. Offsets are relative to CODE.
use against_rome_data::lzss::decompress_pfil;
use against_rome_scenario::BciImage;
use std::{env, fs, io::Write, process::ExitCode};

type Result<T> = std::result::Result<T, String>;
const USAGE: &str = "bcitool - reader/disassembler for Against Rome `BCI0` script bytecode.\n\nRecreates the throwaway helper referenced in\n`docs/reverse-engineering/bci0-opcodes.md` (which noted the original tool was\nnever committed). Pure standard library; no third-party deps.\n\nThe game ships each script as a `PFIL@` LZSS-wrapped `BCI0` container under\n`SYSTEM/CLAK/SCRIPT/*.bci` and `MAPS/**/SCRIPT/*.bci`. This tool decompresses\nthe wrapper, parses the container, and disassembles the code stream using the\nopcode table documented in `docs/reverse-engineering/bci0-opcodes.md`.\n\nContainer layout (little-endian 32-bit words; see the doc for provenance):\n  0x00 \"BCI0\"        0x08 codeSize      0x10 blobLen     0x14 symCount\n  0x24 code stream (codeSize bytes)  -> all \"decompressed offset\" refs are\n                                        relative to this base (byte 0 = 0x24)\n  then \"SYMBCONS\" + blob(blobLen) + \"CIDX\" + symCount*int32 offsets\n  then \"VAR \" ... \"VIDX\" ... (locals metadata, not decoded)\n\nSubcommands:\n  dec   <file> [out]          PFIL decompress to raw BCI0 bytes\n  syms  <file>                dump external symbol table (index -> name)\n  calls <file>               scan pushsym(128)+argc+call(86) sites, resolve names\n  find  <file> <symbol>       list code offsets that pushsym a given symbol\n  funcs <file>                list function prologue offsets (95 75 121 74 94)\n  dis   <file> [start] [end]  linear disassembly (offsets are code-stream bytes)\n\nOffsets printed by `dis`/`find`/`calls` are code-stream relative (add 0x24 to\nget the byte offset inside the decompressed `.dec` container file).\n";

fn one_arg(op: u32) -> bool {
    matches!(
        op,
        64 | 65
            | 66
            | 68
            | 69
            | 70
            | 73
            | 76
            | 77
            | 78
            | 80
            | 81
            | 82
            | 83
            | 84
            | 90
            | 91
            | 92
            | 93
            | 112
            | 113
            | 114
            | 115
            | 116
            | 117
            | 118
            | 120
            | 128
            | 129
            | 160
    )
}
fn opname(op: u32) -> String {
    match op {
        32 => "add",
        33 => "sub",
        34 => "mul",
        35 => "div",
        37 => "bitop",
        40 => "or",
        41 => "or2",
        44 => "and/comb",
        66 => "pushlit",
        71 => "pop",
        73 => "argc",
        74 => "p74",
        75 => "p75",
        77 => "pushglobref",
        80 => "pushsym80",
        81 => "pushvar",
        82 => "storevar",
        86 => "call",
        90 => "arrbase",
        91 => "arrstore",
        94 => "p94",
        95 => "p95",
        96 => "cmp0",
        98 => "lt",
        99 => "le",
        100 => "gt",
        101 => "ge",
        102 => "eq",
        103 => "ne",
        112 => "jmp",
        113 => "jlt",
        114 => "jle",
        115 => "jgt",
        116 => "jge",
        117 => "jz",
        118 => "jnz",
        120 => "callint",
        121 => "ret",
        128 => "pushsym",
        160 => "arrcreate",
        163 => "arrrd",
        164 => "arrwr",
        _ => return format!("op{op}"),
    }
    .into()
}
struct Bci {
    words: Vec<i32>,
    names: Vec<String>,
}
fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}
impl Bci {
    fn parse(data: &[u8]) -> Result<Self> {
        // Reuse the full parser when possible; the Python reader intentionally ignores
        // version, CODE, locals and entrypoints, so retain its relaxed layout too.
        if let Ok(image) = BciImage::parse(data) {
            let names = (0..image.constant_offsets.len())
                .map(|i| image.constant(i).map(latin1))
                .collect::<Result<_>>()?;
            return Ok(Self {
                words: image.code,
                names,
            });
        }
        fn word(d: &[u8], p: usize) -> Result<usize> {
            let b = d
                .get(p..p.checked_add(4).ok_or("BCI length overflow")?)
                .ok_or("truncated BCI")?;
            Ok(u32::from_le_bytes(b.try_into().unwrap()) as usize)
        }
        if !data.starts_with(b"BCI0") {
            return Err("not a BCI0 container".into());
        }
        let code_len = word(data, 8)?;
        let blob_len = word(data, 16)?;
        let count = word(data, 20)?;
        let p = 36usize.checked_add(code_len).ok_or("BCI length overflow")?;
        let q = p
            .checked_add(8)
            .and_then(|p| p.checked_add(blob_len))
            .ok_or("BCI length overflow")?;
        if data.get(p..p.saturating_add(8)) != Some(b"SYMBCONS") {
            return Err("missing SYMBCONS".into());
        }
        if data.get(q..q.saturating_add(4)) != Some(b"CIDX") {
            return Err("missing CIDX".into());
        }
        let blob = &data[p + 8..q];
        let offsets_len = count
            .checked_mul(4)
            .and_then(|n| n.checked_add(4))
            .ok_or("BCI length overflow")?;
        let indices = data
            .get(q..q.checked_add(offsets_len).ok_or("BCI length overflow")?)
            .ok_or("truncated CIDX")?;
        let mut names = Vec::new();
        for i in 0..count {
            let start = word(indices, 4 + i * 4)?;
            // Python's find returns -1 when no NUL is present; slicing then drops
            // the final byte. Preserve that behavior even for unusual blobs.
            let end = blob
                .get(start..)
                .and_then(|b| b.iter().position(|&c| c == 0))
                .map(|n| start + n)
                .unwrap_or(blob.len().saturating_sub(1));
            names.push(latin1(blob.get(start..end).unwrap_or_default()));
        }
        let words = data
            .get(36..p)
            .ok_or("truncated CODE")?
            .chunks_exact(4)
            .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        Ok(Self { words, names })
    }
    fn name(&self, i: i32) -> &str {
        usize::try_from(i)
            .ok()
            .and_then(|i| self.names.get(i))
            .map(String::as_str)
            .unwrap_or("??")
    }
    fn funcs(&self) -> Vec<usize> {
        self.words
            .windows(5)
            .enumerate()
            .filter_map(|(i, w)| (w == [95, 75, 121, 74, 94]).then_some(i * 4))
            .collect()
    }
    fn calls(&self) -> Result<Vec<(usize, i32)>> {
        let mut result = vec![];
        let mut i = 0;
        while i < self.words.len() {
            let op = self.words[i] as u32;
            if op == 128 {
                result.push((
                    i * 4,
                    *self.words.get(i + 1).ok_or("truncated pushsym operand")?,
                ));
            }
            i += if op == 67 {
                3
            } else if one_arg(op) {
                2
            } else {
                1
            };
        }
        Ok(result)
    }
    fn dis(&self, start: i64, end: i64) -> Result<()> {
        let mut i = start.div_euclid(4);
        let end = end.div_euclid(4);
        // Python permits negative indices. Offsets and branch arithmetic still
        // use the original (possibly negative) index.
        let at = |i: i64| -> Result<i32> {
            let index = if i < 0 {
                self.words.len() as i64 + i
            } else {
                i
            };
            usize::try_from(index)
                .ok()
                .and_then(|i| self.words.get(i))
                .copied()
                .ok_or_else(|| "disassembly offset outside CODE".into())
        };
        while i < end {
            let op = at(i)? as u32;
            let off = i * 4;
            let name = opname(op);
            let (text, step) = if op == 67 {
                (format!("{name} {}, {}", at(i + 1)?, at(i + 2)?), 3)
            } else if one_arg(op) {
                let a = at(i + 1)?;
                let text = match op {
                    128 => format!("pushsym #{a} <{}>", self.name(a)),
                    112..=118 | 120 => {
                        format!("{name} ->{} (op {a})", hex(off + 8 + i64::from(a), 6))
                    }
                    81 => format!("pushvar v{a}"),
                    82 => format!("storevar v{a}"),
                    _ => format!("{name} {a}"),
                };
                (text, 2)
            } else {
                (name, 1)
            };
            println!("{}: {text}", hex(off, 8));
            i += step;
        }
        Ok(())
    }
}
fn hex(n: i64, width: usize) -> String {
    if n < 0 {
        format!(
            "-0x{:0width$x}",
            n.unsigned_abs(),
            width = width.saturating_sub(3)
        )
    } else {
        format!("0x{:0width$x}", n, width = width.saturating_sub(2))
    }
}
fn offset(s: &str) -> Result<i64> {
    let trimmed = s.trim();
    let s = trimmed
        .strip_prefix('-')
        .or_else(|| trimmed.strip_prefix('+'))
        .unwrap_or(trimmed);
    let (radix, digits) = if s.starts_with("0x") || s.starts_with("0X") {
        (16, &s[2..])
    } else if s.starts_with("0o") || s.starts_with("0O") {
        (8, &s[2..])
    } else if s.starts_with("0b") || s.starts_with("0B") {
        (2, &s[2..])
    } else {
        (10, s)
    };
    // Match int(text, 0): one underscore may follow a base prefix, but
    // separators must otherwise occur between digits; decimal 012 is invalid.
    let digits = if radix != 10 {
        digits.strip_prefix('_').unwrap_or(digits)
    } else {
        digits
    };
    let clean = digits.replace('_', "");
    if digits.is_empty()
        || digits.starts_with('_')
        || digits.ends_with('_')
        || digits.contains("__")
        || (radix == 10 && clean.starts_with('0') && clean.bytes().any(|b| b != b'0'))
    {
        return Err(format!("invalid offset: {trimmed}"));
    }
    let value =
        i64::from_str_radix(&clean, radix).map_err(|_| format!("invalid offset: {trimmed}"))?;
    Ok(if trimmed.starts_with('-') {
        -value
    } else {
        value
    })
}
fn run() -> Result<u8> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() < 3 {
        println!("{USAGE}");
        return Ok(1);
    }
    let cmd = args[1].to_string_lossy();
    let raw = fs::read(&args[2]).map_err(|e| e.to_string())?;
    let data = if raw.starts_with(b"PFIL") {
        decompress_pfil(&raw, None).map_err(|e| e.to_string())?
    } else {
        raw
    };
    if cmd == "dec" {
        let out = args.get(3).cloned().unwrap_or_else(|| {
            let mut p = args[2].clone();
            p.push(".dec");
            p
        });
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&out)
            .map_err(|e| format!("{}: {e}", out.to_string_lossy()))?;
        file.write_all(&data).map_err(|e| e.to_string())?;
        println!("wrote {} bytes -> {}", data.len(), out.to_string_lossy());
        return Ok(0);
    }
    let bci = Bci::parse(&data)?;
    match cmd.as_ref() {
        "syms" => {
            for (i, n) in bci.names.iter().enumerate() {
                println!("#{i:<4} {n}");
            }
        }
        "funcs" => {
            let offs = bci.funcs();
            println!(
                "{} functions: {}",
                offs.len(),
                offs.iter()
                    .map(|&o| format!("{o:#x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        "calls" => {
            let starts = bci.funcs();
            for (off, idx) in bci.calls()? {
                let function = starts
                    .iter()
                    .rev()
                    .find(|&&s| s <= off)
                    .map(|&s| format!("{s:#08x}"))
                    .unwrap_or_else(|| "----".into());
                println!(
                    "{off:#08x}  #{idx:<3} {:28} (func {function})",
                    bci.name(idx)
                );
            }
        }
        "find" => {
            let want = args
                .get(3)
                .ok_or("find requires a symbol")?
                .to_string_lossy();
            for (off, idx) in bci.calls()? {
                if idx >= 0
                    && bci
                        .names
                        .get(idx as usize)
                        .is_some_and(|n| n == want.as_ref())
                {
                    println!("{off:#08x}  {}", bci.name(idx));
                }
            }
        }
        "dis" => {
            let start = args
                .get(3)
                .map(|s| offset(&s.to_string_lossy()))
                .transpose()?
                .unwrap_or(0);
            let end = args
                .get(4)
                .map(|s| offset(&s.to_string_lossy()))
                .transpose()?
                .unwrap_or(bci.words.len() as i64 * 4);
            bci.dis(start, end)?;
        }
        _ => {
            println!("unknown command: {cmd}");
            return Ok(1);
        }
    }
    Ok(0)
}
fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
