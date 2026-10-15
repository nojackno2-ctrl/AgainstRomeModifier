#![allow(non_snake_case)]
/// Profile reads are delegated to Win32 in production so decimal, quoting,
/// case-insensitivity and missing-key behavior match GetPrivateProfile APIs.
pub trait Profile {
    fn integer(&self, section: &str, key: &str, fallback: i32) -> i32;
    fn string(&self, section: &str, key: &str, fallback: &str) -> String;
}
impl Config {
    pub fn load(p: &impl Profile) -> Self {
        let mut c = Self::default();
        macro_rules! boolean { ($section:expr, $($f:ident),*) => { $(c.$f = p.integer($section, stringify!($f), c.$f as i32) != 0;)* }; }
        boolean!("general", enabled, verifySignatures, enableHooks);
        boolean!(
            "hooks",
            traceNpcJobs,
            traceNpcActive,
            traceVillage,
            traceCreateUnit,
            traceNpcQuery,
            traceLevelInit,
            traceUnitMax,
            traceOpcodes
        );
        c.argDumpCount = p
            .integer("general", "argDumpCount", c.argDumpCount)
            .clamp(0, 16);
        c.maxLogMegabytes = p.integer("general", "maxLogMegabytes", c.maxLogMegabytes);
        let s = p.string("build", "expectedTimeDateStamp", "0");
        let s = s.trim_start();
        let (negative, s) = if let Some(s) = s.strip_prefix('-') {
            (true, s)
        } else {
            (false, s.strip_prefix('+').unwrap_or(s))
        };
        let s = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s);
        let digits: String = s.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        c.expectedTimeDateStamp = match u32::from_str_radix(&digits, 16) {
            Ok(v) if negative => v.wrapping_neg(),
            Ok(v) => v,
            Err(_) if digits.is_empty() => 0,
            Err(_) => u32::MAX,
        };
        c
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub enabled: bool,
    pub verifySignatures: bool,
    pub enableHooks: bool,
    pub traceNpcJobs: bool,
    pub traceNpcActive: bool,
    pub traceVillage: bool,
    pub traceCreateUnit: bool,
    pub traceNpcQuery: bool,
    pub traceLevelInit: bool,
    pub traceUnitMax: bool,
    pub traceOpcodes: bool,
    pub argDumpCount: i32,
    pub maxLogMegabytes: i32,
    pub expectedTimeDateStamp: u32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            verifySignatures: true,
            enableHooks: false,
            traceNpcJobs: true,
            traceNpcActive: true,
            traceVillage: true,
            traceCreateUnit: true,
            traceNpcQuery: true,
            traceLevelInit: true,
            traceUnitMax: true,
            traceOpcodes: false,
            argDumpCount: 9,
            maxLogMegabytes: 256,
            expectedTimeDateStamp: 0,
        }
    }
}
