//! iced-x86 based replacement for the Capstone calls of the Python probes.
//!
//! Instruction text is produced with the same formatter options as `saved-light-contract`
//! (lower case, `0x` hex, memory widths always shown) and compared with the assertions via
//! `saved_light_contract::canonical`, which normalizes only spelling (numeric radix, whitespace,
//! equivalent conditional mnemonics). Mnemonic, operand order, widths and registers are preserved.

use crate::util::{hex, read_va, R};
use iced_x86::{Decoder, DecoderOptions, Formatter, Instruction, IntelFormatter, MemorySizeOptions};
use std::collections::BTreeMap;

pub fn formatter() -> IntelFormatter {
    let mut formatter = IntelFormatter::new();
    let options = formatter.options_mut();
    options.set_hex_prefix("0x");
    options.set_hex_suffix("");
    options.set_uppercase_hex(false);
    options.set_small_hex_numbers_in_decimal(true);
    options.set_branch_leading_zeros(false);
    options.set_displacement_leading_zeros(false);
    options.set_add_leading_zero_to_hex_numbers(false);
    options.set_show_branch_size(false);
    options.set_signed_immediate_operands(false);
    options.set_use_pseudo_ops(false);
    options.set_memory_size_options(MemorySizeOptions::Always);
    options.set_space_after_operand_separator(true);
    options.set_prefer_st0(false);
    options.set_space_between_memory_add_operators(true);
    formatter
}

/// (mnemonic, operand text) in the Capstone `mnemonic` / `op_str` split.
pub fn format_instruction(formatter: &mut IntelFormatter, ins: &Instruction) -> (String, String) {
    let mut mnemonic = String::new();
    formatter.format_mnemonic(ins, &mut mnemonic);
    let mut operands = Vec::new();
    for index in 0..formatter.operand_count(ins) {
        // iced adds implicit x87 ST0 operands that have no encoded operand.
        if matches!(formatter.get_instruction_operand(ins, index), Ok(None)) {
            continue;
        }
        let mut operand = String::new();
        formatter.format_operand(ins, &mut operand, index).expect("formatter operand index");
        operands.push(operand);
    }
    (mnemonic, operands.join(", "))
}

#[derive(Clone)]
pub struct Ins {
    pub ins: Instruction,
    pub bytes: Vec<u8>,
    pub mnemonic: String,
    pub operands: String,
}

impl Ins {
    pub fn address(&self) -> u64 {
        self.ins.ip()
    }
    /// `f'{address:#x} {bytes.hex()} {mnemonic} {op_str}'`
    pub fn line(&self) -> String {
        format!("{:#x} {} {} {}", self.address(), hex(&self.bytes), self.mnemonic, self.operands)
    }
}

/// Linear decode of `bytes` at `address`; like `Cs.disasm`, stops at the first invalid instruction.
pub fn decode_linear(bytes: &[u8], address: u64) -> Vec<Ins> {
    let mut formatter = formatter();
    let mut decoder = Decoder::with_ip(32, bytes, address, DecoderOptions::NONE);
    let mut out = Vec::new();
    while decoder.can_decode() {
        let position = decoder.position();
        let ins = decoder.decode();
        if ins.is_invalid() {
            break;
        }
        let (mnemonic, operands) = format_instruction(&mut formatter, &ins);
        out.push(Ins { bytes: bytes[position..position + ins.len()].to_vec(), ins, mnemonic, operands });
    }
    out
}

/// Disassembly of several `(address, size)` regions of the EXE.
pub struct Code {
    pub list: Vec<Ins>,
    pub index: BTreeMap<u64, usize>,
}

impl Code {
    pub fn regions(exe: &[u8], regions: &[(u64, usize)]) -> R<Self> {
        let mut list = Vec::new();
        for &(address, size) in regions {
            list.extend(decode_linear(read_va(exe, address, size)?, address));
        }
        let index = list.iter().enumerate().map(|(i, ins)| (ins.address(), i)).collect();
        Ok(Self { list, index })
    }
    pub fn at(&self, address: u64) -> Option<&Ins> {
        self.index.get(&address).map(|&i| &self.list[i])
    }
    /// Asserts mnemonic/operands at `address` (exact instruction boundary, spelling-normalized).
    pub fn check(&self, address: u64, mnemonic: &str, operands: &str) -> R<()> {
        match self.at(address) {
            Some(ins) if same(&ins.mnemonic, &ins.operands, mnemonic, operands) => Ok(()),
            _ => Err(format!("instruction mismatch {address:#x}")),
        }
    }
    pub fn lines(&self) -> Vec<String> {
        self.list.iter().map(Ins::line).collect()
    }
    /// Operand text of every `call` in `[from, to]` whose target text is one of `targets`.
    pub fn calls_in(&self, from: u64, to: u64, targets: &[&str]) -> Vec<String> {
        self.list
            .iter()
            .filter(|i| (from..=to).contains(&i.address()) && i.mnemonic == "call" && targets.contains(&i.operands.as_str()))
            .map(|i| i.operands.clone())
            .collect()
    }
}

pub fn same(m1: &str, o1: &str, m2: &str, o2: &str) -> bool {
    match (saved_light_contract::canonical(m1, o1), saved_light_contract::canonical(m2, o2)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Runs a list of `(address, mnemonic, operands)` assertions.
pub fn check_all(code: &Code, checks: &[(u64, &str, &str)]) -> R<()> {
    for &(address, mnemonic, operands) in checks {
        code.check(address, mnemonic, operands)?;
    }
    Ok(())
}
