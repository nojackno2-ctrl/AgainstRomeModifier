//! CLI compatibility checks against tools/bcitool.py; no installed game required.
use against_rome_scenario::BciImage;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "arm-bcitool-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("create {}: {e}", path.display()),
            }
        }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(command: &str, path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ar-bcitool"))
        .arg(command)
        .arg(path)
        .args(args)
        .env("PYTHONIOENCODING", "utf-8")
        .output()
        .expect("launch ar-bcitool")
}

fn assert_output(command: &str, path: &Path, args: &[&str], expected: &str) {
    let output = run(command, path, args);
    assert!(
        output.status.success(),
        "{command} {args:?}: {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stderr, b"", "{command} {args:?}");
    // The port writes LF even on Windows; assert bytes, including padding/newlines.
    assert_eq!(output.stdout, expected.as_bytes(), "{command} {args:?}");
}

fn image(code: &[i32]) -> Vec<u8> {
    BciImage {
        code: code.to_vec(),
        symbols: vec![],
        constants: b"alpha\0beta\0alpha\0caf\xe9\0\0".to_vec(),
        constant_offsets: vec![0, 6, 11, 17, 22],
        variables: vec![],
        variable_offsets: vec![],
        dtor: 0,
        main: 0,
    }
    .serialize()
}

fn pfil(raw: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 64];
    bytes[..5].copy_from_slice(b"PFIL@");
    against_rome_scenario::bci_script::compress_pfil_literal(raw, &bytes).unwrap()
}

fn variants(dir: &TempDir, raw: &[u8]) -> [PathBuf; 2] {
    [
        dir.write("raw script.bci", raw),
        dir.write("packed script.bci", &pfil(raw)),
    ]
}

fn scan_fixture() -> Vec<u8> {
    image(&[
        128, 0, // 0x00: pushsym without a following call still appears.
        67, 128, 2, // 0x08: both operands must be skipped by the scan.
        66, 128, // 0x14: another operand that resembles pushsym.
        80, 2, // 0x1c: pushsym80 is not a pushsym site.
        95, 75, 121, 74, 94, // 0x24: first function.
        128, 1, 73, 0, 86, // 0x38: conventional call.
        128, 2, // 0x4c: duplicate symbol name at another index.
        128, -1, // 0x54: Python must not use negative name indexing.
        128, 99, // 0x5c: out-of-range symbol.
        95, 75, 121, 74, 94, // 0x64: second function.
        128, 0, 121, // 0x78: attributed to the second function.
    ])
}

#[test]
fn exact_symbol_call_find_function_and_disassembly_output_raw_and_pfil() {
    let dir = TempDir::new();
    for path in variants(&dir, &scan_fixture()) {
        assert_output(
            "syms",
            &path,
            &[],
            "#0    alpha\n#1    beta\n#2    alpha\n#3    café\n#4    \n",
        );
        assert_output("funcs", &path, &[], "2 functions: 0x24 0x64\n");
        assert_output(
            "calls",
            &path,
            &[],
            concat!(
                "0x000000  #0   alpha                        (func ----)\n",
                "0x000038  #1   beta                         (func 0x000024)\n",
                "0x00004c  #2   alpha                        (func 0x000024)\n",
                "0x000054  #-1  ??                           (func 0x000024)\n",
                "0x00005c  #99  ??                           (func 0x000024)\n",
                "0x000078  #0   alpha                        (func 0x000064)\n",
            ),
        );
        assert_output(
            "find",
            &path,
            &["alpha"],
            "0x000000  alpha\n0x00004c  alpha\n0x000078  alpha\n",
        );
        assert_output("find", &path, &["beta"], "0x000038  beta\n");
        for absent in ["ALPHA", "missing", "??", "", "café"] {
            assert_output("find", &path, &[absent], "");
        }
        assert_output("dis", &path, &[], concat!(
            "0x000000: pushsym #0 <alpha>\n",
            "0x000008: op67 128, 2\n",
            "0x000014: pushlit 128\n",
            "0x00001c: pushsym80 2\n",
            "0x000024: p95\n0x000028: p75\n0x00002c: ret\n0x000030: p74\n0x000034: p94\n",
            "0x000038: pushsym #1 <beta>\n0x000040: argc 0\n0x000048: call\n",
            "0x00004c: pushsym #2 <alpha>\n0x000054: pushsym #-1 <??>\n0x00005c: pushsym #99 <??>\n",
            "0x000064: p95\n0x000068: p75\n0x00006c: ret\n0x000070: p74\n0x000074: p94\n",
            "0x000078: pushsym #0 <alpha>\n0x000080: ret\n",
        ));
    }
}

#[test]
fn dec_copies_exact_bytes_and_refuses_explicit_default_and_input_overwrites() {
    let dir = TempDir::new();
    let raw = scan_fixture();
    for (index, path) in variants(&dir, &raw).iter().enumerate() {
        let original = fs::read(path).unwrap();
        let explicit = dir.0.join(format!("explicit {index}.dec"));
        let default = PathBuf::from(format!("{}.dec", path.display()));
        for (out, is_default) in [(&explicit, false), (&default, true)] {
            let out_arg = out.to_str().unwrap();
            let args = if is_default { vec![] } else { vec![out_arg] };
            assert_output(
                "dec",
                path,
                &args,
                &format!("wrote {} bytes -> {}\n", raw.len(), out.display()),
            );
            assert_eq!(fs::read(out).unwrap(), raw);
            fs::write(out, b"keep existing output").unwrap();
            let refused = run("dec", path, &args);
            assert!(!refused.status.success());
            assert_eq!(refused.stdout, b"");
            assert!(!refused.stderr.is_empty());
            assert_eq!(fs::read(out).unwrap(), b"keep existing output");
        }
        let refused = run("dec", path, &[path.to_str().unwrap()]);
        assert!(!refused.status.success());
        assert_eq!(refused.stdout, b"");
        assert!(!refused.stderr.is_empty());
        assert_eq!(fs::read(path).unwrap(), original);
    }
    // dec is a byte copier/decompressor, not a BCI parser.
    let arbitrary = dir.write("arbitrary", b"not BCI\0\xff");
    assert_output(
        "dec",
        &arbitrary,
        &[],
        &format!("wrote 9 bytes -> {}.dec\n", arbitrary.display()),
    );
    assert_eq!(
        fs::read(format!("{}.dec", arbitrary.display())).unwrap(),
        b"not BCI\0\xff"
    );
}

#[test]
fn all_operand_opcodes_signed_values_and_unsigned_unknown_opcodes() {
    let dir = TempDir::new();
    let cases: &[(&[i32], &str)] = &[
        (&[64, -7], "op64 -7"),
        (&[65, -7], "op65 -7"),
        (&[66, i32::MIN], "pushlit -2147483648"),
        (&[67, -8, i32::MAX], "op67 -8, 2147483647"),
        (&[68, -7], "op68 -7"),
        (&[69, -7], "op69 -7"),
        (&[70, -7], "op70 -7"),
        (&[73, -1], "argc -1"),
        (&[76, -7], "op76 -7"),
        (&[77, -7], "pushglobref -7"),
        (&[78, -7], "op78 -7"),
        (&[80, -7], "pushsym80 -7"),
        (&[81, -3], "pushvar v-3"),
        (&[82, i32::MAX], "storevar v2147483647"),
        (&[83, -7], "op83 -7"),
        (&[84, -7], "op84 -7"),
        (&[90, -7], "arrbase -7"),
        (&[91, -7], "arrstore -7"),
        (&[92, -7], "op92 -7"),
        (&[93, -7], "op93 -7"),
        (&[112, -16], "jmp ->-0x008 (op -16)"),
        (&[113, -8], "jlt ->0x0000 (op -8)"),
        (&[114, 0], "jle ->0x0008 (op 0)"),
        (&[115, 12], "jgt ->0x0014 (op 12)"),
        (&[116, i32::MIN], "jge ->-0x7ffffff8 (op -2147483648)"),
        (&[117, i32::MAX], "jz ->0x80000007 (op 2147483647)"),
        (&[118, -9], "jnz ->-0x001 (op -9)"),
        (&[120, 4], "callint ->0x000c (op 4)"),
        (&[128, -1], "pushsym #-1 <??>"),
        (&[128, 99], "pushsym #99 <??>"),
        (&[128, 3], "pushsym #3 <café>"),
        (&[128, 4], "pushsym #4 <>"),
        (&[129, -7], "op129 -7"),
        (&[160, -7], "arrcreate -7"),
        (&[-1], "op4294967295"),
        (&[i32::MIN], "op2147483648"),
        (&[0], "op0"),
    ];
    for &(code, expected) in cases {
        for path in variants(&dir, &image(code)) {
            assert_output("dis", &path, &[], &format!("0x000000: {expected}\n"));
        }
    }
}

#[test]
fn no_operand_opcode_names() {
    let dir = TempDir::new();
    let raw = image(&[
        32, 33, 34, 35, 37, 40, 41, 44, 71, 74, 75, 86, 94, 95, 96, 98, 99, 100, 101, 102, 103,
        121, 163, 164,
    ]);
    for path in variants(&dir, &raw) {
        assert_output(
            "dis",
            &path,
            &[],
            concat!(
                "0x000000: add\n0x000004: sub\n0x000008: mul\n0x00000c: div\n",
                "0x000010: bitop\n0x000014: or\n0x000018: or2\n0x00001c: and/comb\n",
                "0x000020: pop\n0x000024: p74\n0x000028: p75\n0x00002c: call\n",
                "0x000030: p94\n0x000034: p95\n0x000038: cmp0\n0x00003c: lt\n",
                "0x000040: le\n0x000044: gt\n0x000048: ge\n0x00004c: eq\n",
                "0x000050: ne\n0x000054: ret\n0x000058: arrrd\n0x00005c: arrwr\n",
            ),
        );
        assert_output("calls", &path, &[], "");
        assert_output("funcs", &path, &[], "0 functions: \n");
    }
}

#[test]
fn disassembly_offsets_use_python_floor_division_and_do_not_clip_operands() {
    let dir = TempDir::new();
    for path in variants(&dir, &image(&[66, 7, 112, -20, 121])) {
        for args in [["1", "7"], ["0b1", "0o7"], ["+0X1", "0x0_7"]] {
            assert_output("dis", &path, &args, "0x000000: pushlit 7\n");
        }
        assert_output(
            "dis",
            &path,
            &["8"],
            "0x000008: jmp ->-0x004 (op -20)\n0x000010: ret\n",
        );
        assert_output(
            "dis",
            &path,
            &["0x9", "0xc"],
            "0x000008: jmp ->-0x004 (op -20)\n",
        );
        assert_output("dis", &path, &["4", "8"], "0x000004: op7\n");
        assert_output("dis", &path, &["-1", "0"], "-0x00004: ret\n");
        for args in [["8", "8"], ["16", "8"], ["20", "20"], ["0", "0"]] {
            assert_output("dis", &path, &args, "");
        }
    }
}

#[test]
fn funcs_searches_every_word_including_operands_and_final_window() {
    let dir = TempDir::new();
    for path in variants(&dir, &image(&[66, 95, 75, 121, 74, 94])) {
        assert_output("funcs", &path, &[], "1 functions: 0x4\n");
        assert_output("calls", &path, &[], "");
    }
    for path in variants(&dir, &image(&[95, 75, 121, 74])) {
        assert_output("funcs", &path, &[], "0 functions: \n");
    }
}

// Python ignores version/CODE metadata, locals and entrypoints, accepts partial
// code words, and uses blob[start:-1] when a symbol has no terminating NUL.
fn relaxed_fixture() -> Vec<u8> {
    let code: [i32; 10] = [128, 0, 128, 1, 128, 2, 128, 3, 128, 4];
    let mut raw = vec![0; 36];
    raw[..4].copy_from_slice(b"BCI0");
    raw[4..8].copy_from_slice(&17_u32.to_le_bytes());
    raw[8..12].copy_from_slice(&41_u32.to_le_bytes());
    raw[16..20].copy_from_slice(&5_u32.to_le_bytes());
    raw[20..24].copy_from_slice(&5_u32.to_le_bytes());
    raw[32..36].copy_from_slice(b"????");
    for word in code {
        raw.extend(word.to_le_bytes());
    }
    raw.push(0xff);
    raw.extend_from_slice(b"SYMBCONSA\0xyzCIDX");
    for offset in [0_u32, 0, 2, 999, u32::MAX] {
        raw.extend(offset.to_le_bytes());
    }
    raw.extend_from_slice(b"opaque tail");
    raw
}

#[test]
fn relaxed_layout_and_python_symbol_slicing_are_preserved() {
    let dir = TempDir::new();
    let raw = relaxed_fixture();
    assert!(BciImage::parse(&raw).is_err());
    for path in variants(&dir, &raw) {
        assert_output(
            "syms",
            &path,
            &[],
            "#0    A\n#1    A\n#2    xy\n#3    \n#4    \n",
        );
        assert_output("find", &path, &["A"], "0x000000  A\n0x000008  A\n");
        assert_output("find", &path, &["xy"], "0x000010  xy\n");
        assert_output("find", &path, &[""], "0x000018  \n0x000020  \n");
        assert_output("funcs", &path, &[], "0 functions: \n");
        assert_output(
            "calls",
            &path,
            &[],
            concat!(
                "0x000000  #0   A                            (func ----)\n",
                "0x000008  #1   A                            (func ----)\n",
                "0x000010  #2   xy                           (func ----)\n",
                "0x000018  #3                                (func ----)\n",
                "0x000020  #4                                (func ----)\n",
            ),
        );
        assert_output(
            "dis",
            &path,
            &[],
            concat!(
                "0x000000: pushsym #0 <A>\n0x000008: pushsym #1 <A>\n",
                "0x000010: pushsym #2 <xy>\n0x000018: pushsym #3 <>\n0x000020: pushsym #4 <>\n",
            ),
        );
    }
}

#[test]
#[ignore = "optional Python differential oracle; run with --ignored when Python 3 is available"]
fn python_differential_all_commands() {
    let python = ["python3", "python", "py"].into_iter().find(|program| {
        Command::new(program)
            .args(["-c", "import sys; assert sys.version_info.major == 3"])
            .output()
            .is_ok_and(|out| out.status.success())
    });
    let Some(python) = python else {
        eprintln!("skipping differential oracle: Python 3 unavailable");
        return;
    };
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/bcitool.py");
    let dir = TempDir::new();
    for raw in [
        scan_fixture(),
        relaxed_fixture(),
        image(&[66, -8, 112, -20, -1, 121]),
    ] {
        for path in variants(&dir, &raw) {
            for (command, args) in [
                ("syms", vec![]),
                ("calls", vec![]),
                ("funcs", vec![]),
                ("find", vec!["alpha"]),
                ("find", vec![""]),
                ("find", vec!["missing"]),
                ("dis", vec![]),
                ("dis", vec!["1", "7"]),
                ("dis", vec!["-1", "0"]),
            ] {
                let expected = Command::new(python)
                    .arg(&script)
                    .arg(command)
                    .arg(&path)
                    .args(&args)
                    .env("PYTHONIOENCODING", "utf-8")
                    .env("PYTHONDONTWRITEBYTECODE", "1")
                    .output()
                    .unwrap();
                let actual = run(command, &path, &args);
                assert!(
                    expected.status.success(),
                    "Python: {}",
                    String::from_utf8_lossy(&expected.stderr)
                );
                assert_eq!(
                    actual.status.code(),
                    expected.status.code(),
                    "{command} {args:?}"
                );
                // Python's text stream uses CRLF on Windows; compare logical output.
                assert_eq!(
                    String::from_utf8(actual.stdout)
                        .unwrap()
                        .replace("\r\n", "\n"),
                    String::from_utf8(expected.stdout)
                        .unwrap()
                        .replace("\r\n", "\n"),
                    "{command} {args:?}"
                );
                assert_eq!(actual.stderr, expected.stderr, "{command} {args:?}");
            }
            // Both tools use the same fresh path so even the dec status line matches.
            // Remove Python's output before Rust: refusal to overwrite is intentional.
            for explicit in [false, true] {
                let out = if explicit {
                    dir.0.join("oracle output.dec")
                } else {
                    PathBuf::from(format!("{}.dec", path.display()))
                };
                let mut oracle = Command::new(python);
                oracle
                    .arg(&script)
                    .arg("dec")
                    .arg(&path)
                    .env("PYTHONIOENCODING", "utf-8");
                if explicit {
                    oracle.arg(&out);
                }
                let expected = oracle.output().unwrap();
                assert!(expected.status.success());
                let expected_bytes = fs::read(&out).unwrap();
                fs::remove_file(&out).unwrap();
                let args = if explicit {
                    vec![out.to_str().unwrap()]
                } else {
                    vec![]
                };
                let actual = run("dec", &path, &args);
                assert_eq!(actual.status.code(), expected.status.code());
                assert_eq!(
                    String::from_utf8(actual.stdout)
                        .unwrap()
                        .replace("\r\n", "\n"),
                    String::from_utf8(expected.stdout)
                        .unwrap()
                        .replace("\r\n", "\n")
                );
                assert_eq!(actual.stderr, expected.stderr);
                assert_eq!(fs::read(&out).unwrap(), expected_bytes);
                fs::remove_file(&out).unwrap();
            }
        }
    }
}
