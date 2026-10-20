//! Win32 integration. ReadProcessMemory replaces the C++ SEH guarded reads.
#![allow(non_snake_case)]
#![cfg_attr(not(target_arch = "x86"), allow(dead_code))]
use crate::{
    config::{Config, Profile},
    detour, pe,
    targets::{self, Memory, Target},
    tracelog::TraceLog,
};
#[cfg(target_arch = "x86")]
use std::ffi::CString;
use std::{
    ffi::c_void,
    fs::{File, OpenOptions},
    path::PathBuf,
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Mutex,
    },
};
type Handle = *mut c_void;
#[link(name = "kernel32")]
extern "system" {
    fn GetSystemDirectoryW(buf: *mut u16, size: u32) -> u32;
    fn LoadLibraryW(path: *const u16) -> Handle;
    fn GetProcAddress(module: Handle, name: *const u8) -> *mut c_void;
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn GetModuleFileNameW(module: Handle, buf: *mut u16, size: u32) -> u32;
    fn DisableThreadLibraryCalls(module: Handle) -> i32;
    fn CreateThread(
        attr: Handle,
        size: usize,
        start: unsafe extern "system" fn(Handle) -> u32,
        arg: Handle,
        flags: u32,
        id: *mut u32,
    ) -> Handle;
    fn CloseHandle(h: Handle) -> i32;
    fn GetCurrentThreadId() -> u32;
    fn GetCurrentProcess() -> Handle;
    fn ReadProcessMemory(
        process: Handle,
        address: *const c_void,
        out: *mut c_void,
        n: usize,
        read: *mut usize,
    ) -> i32;
    fn VirtualAlloc(address: Handle, size: usize, kind: u32, protect: u32) -> Handle;
    fn VirtualFree(address: Handle, size: usize, kind: u32) -> i32;
    fn VirtualProtect(address: Handle, size: usize, protect: u32, old: *mut u32) -> i32;
    fn FlushInstructionCache(process: Handle, address: *const c_void, size: usize) -> i32;
    fn GetPrivateProfileIntW(
        section: *const u16,
        key: *const u16,
        fallback: i32,
        file: *const u16,
    ) -> u32;
    fn GetPrivateProfileStringW(
        section: *const u16,
        key: *const u16,
        fallback: *const u16,
        out: *mut u16,
        cap: u32,
        file: *const u16,
    ) -> u32;
}
include!(concat!(env!("OUT_DIR"), "/exports.rs"));
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
struct Ini(Vec<u16>);
impl Profile for Ini {
    fn integer(&self, s: &str, k: &str, d: i32) -> i32 {
        unsafe {
            GetPrivateProfileIntW(wide(s).as_ptr(), wide(k).as_ptr(), d, self.0.as_ptr()) as i32
        }
    }
    fn string(&self, s: &str, k: &str, d: &str) -> String {
        let mut b = [0u16; 32];
        let n = unsafe {
            GetPrivateProfileStringW(
                wide(s).as_ptr(),
                wide(k).as_ptr(),
                wide(d).as_ptr(),
                b.as_mut_ptr(),
                32,
                self.0.as_ptr(),
            )
        };
        String::from_utf16_lossy(&b[..n as usize])
    }
}
struct Process;
impl Memory for Process {
    fn read(&self, address: u32, out: &mut [u8]) -> bool {
        unsafe {
            let mut n = 0;
            ReadProcessMemory(
                GetCurrentProcess(),
                address as usize as _,
                out.as_mut_ptr() as _,
                out.len(),
                &mut n,
            ) != 0
                && n == out.len()
        }
    }
}
static LOG: Mutex<Option<TraceLog<File>>> = Mutex::new(None);
static BASE: AtomicU32 = AtomicU32::new(0);
fn log(category: &str, message: &str) {
    if let Ok(mut l) = LOG.lock() {
        if let Some(l) = l.as_mut() {
            let _ = l.log(unsafe { GetCurrentThreadId() }, category, message);
        }
    }
}
#[cfg(target_arch = "x86")]
unsafe fn load_proxy() -> bool {
    let mut path = [0u16; 260];
    let n = GetSystemDirectoryW(path.as_mut_ptr(), 260) as usize;
    if n == 0 || n + 11 >= 260 {
        return false;
    }
    let mut p = path[..n].to_vec();
    p.extend(wide("\\winmm.dll"));
    let module = LoadLibraryW(p.as_ptr());
    if module.is_null() {
        return false;
    }
    for (i, name) in EXPORT_NAMES.iter().enumerate() {
        let addr = if i == 0 {
            GetProcAddress(module, 2usize as *const u8)
        } else {
            let s = CString::new(*name).unwrap();
            GetProcAddress(module, s.as_ptr() as _)
        };
        if addr.is_null() {
            return false;
        }
        WINMM_POINTERS[i] = addr as usize;
    }
    true
}
// Own the descriptor for as long as generated machine code references it.
struct Installed {
    target: usize,
    original: Vec<u8>,
    trampoline: usize,
    stub: usize,
    _descriptor: Box<Target>,
}
static HOOKS: Mutex<Vec<Installed>> = Mutex::new(Vec::new());
// Set once the process is detaching so a late init thread cannot patch code
// that points into a module that is about to be unmapped.
static SHUTDOWN: AtomicBool = AtomicBool::new(false);
unsafe extern "C" fn on_hit(desc: *const Target, rb: *const detour::RegBank) {
    let _ = std::panic::catch_unwind(|| {
        if desc.is_null() || rb.is_null() {
            return;
        }
        let t = &*desc;
        let count = t.argc.max(usize::from(t.name == "bciop")) + 1;
        let mut b = vec![0u8; count * 4];
        if !Process.read((rb as usize).wrapping_add(36) as u32, &mut b) {
            return;
        }
        let cs: Vec<u32> = b
            .chunks_exact(4)
            .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
            .collect();
        #[cfg(all(test, target_arch = "x86"))]
        if let Ok(mut hit) = TEST_HIT.lock() {
            *hit = cs.clone();
        }
        if let Some((c, s)) = targets::format_hit(t, &cs, BASE.load(Ordering::Relaxed), &Process) {
            log(c, &s);
        }
    });
}
unsafe fn install(t: Target, verify_signatures: bool) -> bool {
    // A poisoned list is still accurate: entries are pushed only after the patch.
    let mut hooks = HOOKS.lock().unwrap_or_else(|e| e.into_inner());
    if SHUTDOWN.load(Ordering::SeqCst) || hooks.len() >= 32 {
        return false;
    }
    let address = pe::rebase(t.va, BASE.load(Ordering::Relaxed));
    let mut bytes = [0u8; 32];
    if !Process.read(address, &mut bytes)
        || (verify_signatures && !detour::verify_signature(&bytes, t.signature, t.mask))
    {
        log(
            "hook",
            &format!("SKIP {}: signature mismatch or unreadable target", t.name),
        );
        return false;
    }
    let n = match detour::stolen_length(&bytes) {
        Ok(n) => n,
        Err(e) => {
            log("hook", &format!("SKIP {}: {e}", t.name));
            return false;
        }
    };
    let tramp = VirtualAlloc(ptr::null_mut(), n + 5, 0x3000, 0x40);
    if tramp.is_null() {
        return false;
    }
    let stub = VirtualAlloc(ptr::null_mut(), 32, 0x3000, 0x40);
    if stub.is_null() {
        VirtualFree(tramp, 0, 0x8000);
        return false;
    }
    let descriptor = Box::new(t);
    let tr = detour::trampoline(&bytes, address, tramp as usize as u32).unwrap();
    let st = detour::stub(
        stub as usize as u32,
        &*descriptor as *const _ as usize as u32,
        on_hit as *const () as usize as u32,
        tramp as usize as u32,
    );
    ptr::copy_nonoverlapping(tr.as_ptr(), tramp as *mut u8, tr.len());
    ptr::copy_nonoverlapping(st.as_ptr(), stub as *mut u8, st.len());
    FlushInstructionCache(GetCurrentProcess(), tramp, tr.len());
    FlushInstructionCache(GetCurrentProcess(), stub, st.len());
    let mut old = 0;
    if VirtualProtect(address as usize as _, n, 0x40, &mut old) == 0 {
        VirtualFree(stub, 0, 0x8000);
        VirtualFree(tramp, 0, 0x8000);
        return false;
    }
    let p = detour::patch(address, stub as usize as u32, n);
    ptr::copy_nonoverlapping(p.as_ptr(), address as usize as *mut u8, n);
    let mut ignored = 0;
    VirtualProtect(address as usize as _, n, old, &mut ignored);
    FlushInstructionCache(GetCurrentProcess(), address as usize as _, n);
    log(
        "hook",
        &format!(
            "OK   {} at {address:08X} (stole {n} bytes)",
            descriptor.name
        ),
    );
    hooks.push(Installed {
        target: address as usize,
        original: bytes[..n].to_vec(),
        trampoline: tramp as usize,
        stub: stub as usize,
        _descriptor: descriptor,
    });
    true
}
unsafe fn remove_all() {
    // Never strand patches because another thread panicked while holding the lock.
    let mut hooks = HOOKS.lock().unwrap_or_else(|e| e.into_inner());
    hooks.retain(|h| {
        let mut old = 0;
        if VirtualProtect(h.target as _, h.original.len(), 0x40, &mut old) == 0 {
            return true;
        }
        ptr::copy_nonoverlapping(h.original.as_ptr(), h.target as *mut u8, h.original.len());
        let mut ignored = 0;
        VirtualProtect(h.target as _, h.original.len(), old, &mut ignored);
        FlushInstructionCache(GetCurrentProcess(), h.target as _, h.original.len());
        VirtualFree(h.trampoline as _, 0, 0x8000);
        VirtualFree(h.stub as _, 0, 0x8000);
        false
    });
}
// Final teardown: block further installs, then restore everything installed so far.
unsafe fn shutdown() {
    {
        let _hooks = HOOKS.lock().unwrap_or_else(|e| e.into_inner());
        SHUTDOWN.store(true, Ordering::SeqCst);
    }
    remove_all();
}
unsafe extern "system" fn init(_: Handle) -> u32 {
    let mut path = [0u16; 260];
    let n = GetModuleFileNameW(ptr::null_mut(), path.as_mut_ptr(), 260) as usize;
    if n == 0 || n >= 260 {
        return 0;
    }
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let mut directory = PathBuf::from(std::ffi::OsString::from_wide(&path[..n]));
    directory.pop();
    let ini: Vec<u16> = directory
        .join("argm_trace.ini")
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let cfg = Config::load(&Ini(ini));
    if !cfg.enabled {
        return 0;
    }
    use std::os::windows::fs::OpenOptionsExt;
    let file = match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .share_mode(1)
        .open(directory.join("argm_trace.log"))
    {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let max_bytes = if cfg.maxLogMegabytes > 0 {
        Some((cfg.maxLogMegabytes as u64) * 1024 * 1024)
    } else {
        None
    };
    let mut writer = match TraceLog::with_max_bytes(file, max_bytes) {
        Ok(l) => l,
        Err(_) => return 0,
    };
    if writer
        .raw("==== argm-trace: Against Rome runtime flight recorder (winmm proxy) ====")
        .is_err()
    {
        return 0;
    }
    match LOG.lock() {
        Ok(mut l) => *l = Some(writer),
        Err(_) => return 0,
    }
    log(
        "init",
        &format!(
            "log opened, verifySignatures={} enableHooks={}",
            cfg.verifySignatures as u8, cfg.enableHooks as u8
        ),
    );
    let base = GetModuleHandleW(ptr::null()) as usize as u32;
    BASE.store(base, Ordering::Relaxed);
    let mut dos = [0u8; 64];
    if !Process.read(base, &mut dos) {
        return 0;
    }
    let offset = u32::from_le_bytes(dos[60..64].try_into().unwrap()) as usize;
    if offset > 1024 * 1024 {
        log("build", "invalid PE header offset");
        return 0;
    }
    let mut header = vec![0u8; offset + 84];
    if !Process.read(base, &mut header) {
        return 0;
    }
    let fp = match pe::fingerprint(&header) {
        Some(f) => f,
        None => {
            log("build", "invalid PE32 fingerprint");
            return 0;
        }
    };
    log("build",&format!("module={base:08X} preferred={:08X} delta={:08X} TimeDateStamp={:08X} SizeOfImage={:08X} entry={:08X}",fp.preferred_base,base.wrapping_sub(0x400000),fp.timestamp,fp.image_size,fp.entry));
    let mut ep = [0u8; 12];
    if Process.read(base.wrapping_add(fp.entry), &mut ep) {
        log(
            "build",
            &format!(
                "entry bytes: {}",
                ep.iter()
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        );
    }
    if fp.preferred_base != 0x400000 {
        log(
            "build",
            "WARNING: PE ImageBase differs from documented base 00400000",
        );
    }
    if !cfg.enableHooks {
        log(
            "init",
            "LOG-ONLY MODE: hook installation disabled (enableHooks=0).",
        );
        return 0;
    }
    if !pe::build_matches(cfg.expectedTimeDateStamp, fp.timestamp) {
        log("build", "BUILD MISMATCH: skipping all address-based hooks.");
        return 0;
    }
    if cfg.expectedTimeDateStamp == 0 {
        log(
            "build",
            "No expectedTimeDateStamp configured: only signature-verified faction hooks enabled.",
        );
    }
    let ts = targets::selected(&cfg);
    let total = ts.len();
    let mut ok = 0;
    for t in ts {
        if install(t, cfg.verifySignatures) {
            ok += 1;
        }
    }
    log("hook", &format!("installed {ok} of {total} hooks"));
    0
}
#[cfg(target_arch = "x86")]
unsafe extern "system" fn dll_main(module: Handle, reason: u32, _reserved: Handle) -> i32 {
    match reason {
        1 => {
            DisableThreadLibraryCalls(module);
            if !load_proxy() {
                return 0;
            }
            let h = CreateThread(
                ptr::null_mut(),
                0,
                init,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
            );
            if !h.is_null() {
                CloseHandle(h);
            }
        }
        0 => {
            shutdown();
            log("shutdown", "process detach");
            if let Ok(mut l) = LOG.lock() {
                if let Some(l) = l.as_mut() {
                    let _ = l.close_synced(|f| f.sync_all());
                }
                *l = None;
            }
        }
        _ => (),
    }
    1
}
// A COFF symbol, rather than #[no_mangle], avoids adding DllMain to the public
// winmm export table. The CRT calls this stdcall entry point on x86.
#[cfg(target_arch = "x86")]
core::arch::global_asm!(".global \"_DllMain@12\"\n\"_DllMain@12\":\njmp {entry}", entry = sym dll_main);

#[cfg(all(test, target_arch = "x86"))]
static TEST_HIT: Mutex<Vec<u32>> = Mutex::new(Vec::new());
#[cfg(test)]
static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn win32_profile_missing_file_keeps_defaults() {
        let c = Config::load(&Ini(wide(
            "Z:\\__arm_trace_missing_8e8d478b__\\argm_trace.ini",
        )));
        assert_eq!(c, Config::default());
    }
    #[test]
    fn unreadable_process_address_returns_false_without_fault() {
        assert!(!Process.read(0, &mut [0; 4]));
    }
    fn fake_hook(buf: &mut [u8; 8], original: [u8; 5]) -> Installed {
        unsafe {
            let tramp = VirtualAlloc(ptr::null_mut(), 16, 0x3000, 0x40) as usize;
            let stub = VirtualAlloc(ptr::null_mut(), 16, 0x3000, 0x40) as usize;
            assert!(tramp != 0 && stub != 0);
            buf[..5].copy_from_slice(&[0xe9, 1, 2, 3, 4]);
            Installed {
                target: buf.as_mut_ptr() as usize,
                original: original.to_vec(),
                trampoline: tramp,
                stub,
                _descriptor: Box::new(Target {
                    name: "fake",
                    va: 0,
                    argc: 0,
                    signature: &[],
                    mask: b"",
                }),
            }
        }
    }
    #[test]
    fn remove_all_restores_bytes_even_when_hook_list_is_poisoned() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut buf = [0u8; 8];
        let h = fake_hook(&mut buf, [1, 2, 3, 4, 5]);
        HOOKS.lock().unwrap_or_else(|e| e.into_inner()).push(h);
        let _ = std::thread::spawn(|| {
            let _g = HOOKS.lock().unwrap_or_else(|e| e.into_inner());
            panic!("poison");
        })
        .join();
        assert!(HOOKS.is_poisoned());
        unsafe { remove_all() };
        assert_eq!(&buf[..5], &[1, 2, 3, 4, 5]);
        assert!(HOOKS.lock().unwrap_or_else(|e| e.into_inner()).is_empty());
        // Repeated teardown is a harmless no-op.
        unsafe { remove_all() };
        HOOKS.clear_poison();
    }
    #[test]
    fn install_after_shutdown_is_refused_and_shutdown_is_repeatable() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut buf = [0u8; 8];
        let h = fake_hook(&mut buf, [9, 8, 7, 6, 5]);
        HOOKS.lock().unwrap_or_else(|e| e.into_inner()).push(h);
        unsafe {
            shutdown();
            shutdown();
            assert_eq!(&buf[..5], &[9, 8, 7, 6, 5]);
            // A late init thread must not patch anything once detach has begun.
            let t = Target {
                name: "faction",
                va: buf.as_ptr() as usize as u32,
                argc: 0,
                signature: &[],
                mask: b"",
            };
            assert!(!install(t, false));
        }
        assert!(HOOKS.lock().unwrap_or_else(|e| e.into_inner()).is_empty());
        SHUTDOWN.store(false, Ordering::SeqCst);
    }
    #[cfg(target_arch = "x86")]
    #[test]
    fn genuine_system_winmm_resolves_every_export() {
        unsafe {
            let mut path = [0u16; 260];
            let n = GetSystemDirectoryW(path.as_mut_ptr(), 260) as usize;
            assert!(n > 0 && n + 11 < 260);
            let mut p = path[..n].to_vec();
            p.extend(wide("\\winmm.dll"));
            let module = LoadLibraryW(p.as_ptr());
            assert!(!module.is_null());
            assert_eq!(EXPORT_COUNT, 193);
            for (i, name) in EXPORT_NAMES.iter().enumerate() {
                let s = std::ffi::CString::new(*name).unwrap();
                let addr = GetProcAddress(
                    module,
                    if i == 0 {
                        2usize as *const u8
                    } else {
                        s.as_ptr() as _
                    },
                );
                assert!(!addr.is_null(), "missing winmm export {name}");
            }
        }
    }
    #[test]
    fn genuine_x86_winmm_file_matches_every_export_name_and_ordinal() {
        let directory = std::env::var_os("WINDIR").expect("Windows directory");
        let path = PathBuf::from(directory).join("SysWOW64").join("winmm.dll");
        let bytes = std::fs::read(&path).expect("read genuine x86 winmm.dll");
        let actual = pe::exports(&bytes).expect("parse genuine x86 winmm exports");
        let expected: Vec<_> = include_str!("../winmm.def")
            .lines()
            .filter_map(|l| {
                let (left, ordinal) = l.split_once('@')?;
                let ordinal = ordinal.split_whitespace().next()?.parse::<u32>().ok()?;
                Some(pe::Export {
                    ordinal,
                    name: if l.contains("NONAME") {
                        None
                    } else {
                        Some(left.split('=').next()?.trim().to_owned())
                    },
                })
            })
            .collect();
        assert_eq!(actual, expected);
    }
    // Requires the real i686 standard library/toolchain. Never executes x86
    // machine code on the x64 host fallback.
    #[cfg(target_arch = "x86")]
    #[test]
    fn real_x86_detour_preserves_return_captures_arguments_and_restores_bytes() {
        let _lock = TEST_LOCK.lock().unwrap();
        unsafe {
            remove_all();
            TEST_HIT.lock().unwrap().clear();
            let p = VirtualAlloc(ptr::null_mut(), 32, 0x3000, 0x40);
            assert!(!p.is_null());
            let code = [0x8b, 0x44, 0x24, 0x04, 0x90, 0x83, 0xc0, 0x07, 0xc3];
            ptr::copy_nonoverlapping(code.as_ptr(), p as *mut u8, code.len());
            FlushInstructionCache(GetCurrentProcess(), p, code.len());
            let call: unsafe extern "C" fn(u32) -> u32 = std::mem::transmute(p);
            assert_eq!(call(5), 12);
            BASE.store(0x400000, Ordering::Relaxed);
            assert!(install(
                Target {
                    name: "faction",
                    va: p as u32,
                    argc: 1,
                    signature: &[0x8b, 0x44, 0x24, 0x04, 0x90],
                    mask: b"xxxxx"
                },
                true
            ));
            assert_eq!(call(20), 27);
            assert_eq!(TEST_HIT.lock().unwrap()[1], 20);
            assert_ne!(TEST_HIT.lock().unwrap()[0], 0);
            assert_eq!(call(99), 106);
            assert_eq!(TEST_HIT.lock().unwrap()[1], 99);
            remove_all();
            assert_eq!(
                std::slice::from_raw_parts(p as *const u8, code.len()),
                &code
            );
            assert_eq!(call(7), 14);
            assert_eq!(TEST_HIT.lock().unwrap()[1], 99);
            VirtualFree(p, 0, 0x8000);
        }
    }

    #[cfg(target_arch = "x86")]
    #[test]
    fn real_x86_multi_arg_detour_preserves_regs_captures_all_args_and_restores() {
        let _lock = TEST_LOCK.lock().unwrap();
        unsafe {
            remove_all();
            TEST_HIT.lock().unwrap().clear();
            let p = VirtualAlloc(ptr::null_mut(), 64, 0x3000, 0x40);
            assert!(!p.is_null());
            // Target function mimicking s_addNPCJob_createUnit(team, mode, a3, a4, a5, cntLo, cntHi, a8, a9):
            // 53 56 57 55               push ebx; push esi; push edi; push ebp
            // 8B 5C 24 14               mov ebx, [esp + 0x14]  (arg1 = team)
            // 8B 74 24 28               mov esi, [esp + 0x28]  (arg6 = cntLo)
            // 69 DB E8 03 00 00         imul ebx, ebx, 1000
            // 01 F3                     add ebx, esi
            // 89 D8                     mov eax, ebx
            // 5D 5F 5E 5B               pop ebp; pop edi; pop esi; pop ebx
            // C3                        ret
            static CODE: [u8; 27] = [
                0x53, 0x56, 0x57, 0x55, 0x8b, 0x5c, 0x24, 0x14, 0x8b, 0x74, 0x24, 0x28, 0x69, 0xdb,
                0xe8, 0x03, 0x00, 0x00, 0x01, 0xf3, 0x89, 0xd8, 0x5d, 0x5f, 0x5e, 0x5b, 0xc3,
            ];
            ptr::copy_nonoverlapping(CODE.as_ptr(), p as *mut u8, CODE.len());
            FlushInstructionCache(GetCurrentProcess(), p, CODE.len());

            type NpcJobFn =
                unsafe extern "C" fn(i32, i32, i32, i32, i32, i32, i32, i32, i32) -> i32;
            let call: NpcJobFn = std::mem::transmute(p);
            assert_eq!(call(5, 1, 4, 0, 0, 20, 20, 8, 3), 5020);

            BASE.store(0x400000, Ordering::Relaxed);
            assert!(install(
                Target {
                    name: "npcjob",
                    va: p as u32,
                    argc: 9,
                    signature: &CODE[..8],
                    mask: b"xxxxxxxx",
                },
                true,
            ));

            // Verify first call with 9 arguments
            assert_eq!(call(5, 1, 4, 0, 0, 20, 20, 8, 3), 5020);
            let hit1 = TEST_HIT.lock().unwrap().clone();
            assert_eq!(hit1.len(), 10); // ret addr + 9 args
            assert_ne!(hit1[0], 0); // ret addr into caller
            assert_eq!(&hit1[1..], &[5, 1, 4, 0, 0, 20, 20, 8, 3]);

            // Verify second call with different arguments
            assert_eq!(call(3, 3, 9, 1, 2, 15, 40, 0, 0), 3015);
            let hit2 = TEST_HIT.lock().unwrap().clone();
            assert_eq!(&hit2[1..], &[3, 3, 9, 1, 2, 15, 40, 0, 0]);

            // Remove detour and verify clean restoration
            remove_all();
            assert_eq!(
                std::slice::from_raw_parts(p as *const u8, CODE.len()),
                &CODE
            );
            assert_eq!(call(7, 0, 0, 0, 0, 99, 0, 0, 0), 7099);
            // Verify no new hit recorded after removal
            assert_eq!(
                &TEST_HIT.lock().unwrap()[1..],
                &[3, 3, 9, 1, 2, 15, 40, 0, 0]
            );

            VirtualFree(p, 0, 0x8000);
        }
    }
}
