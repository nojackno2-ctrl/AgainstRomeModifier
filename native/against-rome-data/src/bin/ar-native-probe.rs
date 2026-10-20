//! Explicit-path Windows DLL audit. No fallback DLL or linked-in implementation is used.
//! Replaces ArNativeProbe in tools/build-native-data.ps1; does not deploy files.
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

const EXPORTS: &[&str] = &[
    "ar_light_abi_version",
    "ar_light_snapshot_struct_size",
    "ar_light_error_message",
    "ar_light_pool_open",
    "ar_light_pool_free",
    "ar_light_pool_active_count",
    "ar_light_pool_snapshot",
    "ar_light_pool_snapshot_any",
    "ar_light_pool_copy_payload",
    "ar_light_pool_copy_pfil_header",
    "ar_particle_pool_abi_version",
    "ar_particle_pool_snapshot_struct_size",
    "ar_particle_pool_record_size",
    "ar_particle_pool_state_size",
    "ar_particle_pool_child_size",
    "ar_particle_pool_payload_size",
    "ar_particle_pool_error_message",
    "ar_particle_pool_open",
    "ar_particle_pool_free",
    "ar_particle_pool_active_count",
    "ar_particle_pool_snapshot",
    "ar_particle_pool_snapshot_any",
    "ar_particle_pool_child_any",
    "ar_particle_pool_validate_renderable_count",
    "ar_particle_pool_copy_payload",
    "ar_particle_pool_copy_pfil_header",
    "ar_particle_pool_copy_input_sha256",
    "ar_particle_pixel_abi_version",
    "ar_particle_pixel_row",
    "ar_alr_scanline_abi_version",
    "ar_alr_decode_argb",
    "ar_particle_definition_abi_version",
    "ar_particle_definition_struct_size",
    "ar_particle_definition_open",
    "ar_particle_definition_free",
    "ar_particle_definition_query",
    "ar_particle_definition_load_utf8",
    "ar_particle_definition_source_count",
    "ar_particle_definition_lifetime_frame",
    "ar_particle_definition_saved_frames",
    "ar_particle_frame_candidate_struct_size",
    "ar_particle_definition_saved_frames_translated",
    "ar_particle_saved_object_mode",
    "ar_particle_saved_object_mode_struct_size",
    "ar_particle_definition_primary_mode",
];
const U32_QUERIES: &[(&str, u32)] = &[
    ("ar_light_abi_version", 1),
    ("ar_light_snapshot_struct_size", 120),
    ("ar_particle_pool_abi_version", 1),
    ("ar_particle_pool_snapshot_struct_size", 24),
    ("ar_particle_pixel_abi_version", 1),
    ("ar_alr_scanline_abi_version", 1),
    ("ar_particle_definition_abi_version", 1),
    ("ar_particle_definition_struct_size", 44),
    ("ar_particle_frame_candidate_struct_size", 40),
    ("ar_particle_saved_object_mode_struct_size", 24),
];
const SIZE_QUERIES: &[(&str, usize)] = &[
    ("ar_particle_pool_record_size", 2252),
    ("ar_particle_pool_state_size", 16),
    ("ar_particle_pool_child_size", 35),
    ("ar_particle_pool_payload_size", 2322444),
];
type PixelRow = unsafe extern "C" fn(
    *const u8,
    usize,
    *const u32,
    usize,
    *const u32,
    u32,
    u32,
    *mut u32,
    usize,
) -> i32;
trait Api {
    fn resolve(&self, name: &str) -> Result<(), String>;
    fn u32_query(&self, name: &str) -> Result<u32, String>;
    fn size_query(&self, name: &str) -> Result<usize, String>;
    fn pixel(&self) -> Result<PixelRow, String>;
}
fn audit(api: &impl Api) -> Result<(), String> {
    // Resolve the entire legacy export surface before calling into the DLL.
    for name in EXPORTS {
        api.resolve(name)?;
    }
    for &(name, expected) in U32_QUERIES {
        let actual = api.u32_query(name)?;
        if actual != expected {
            return Err(format!("{name}: got {actual}, expected {expected}"));
        }
    }
    for &(name, expected) in SIZE_QUERIES {
        let actual = api.size_query(name)?;
        if actual != expected {
            return Err(format!("{name}: got {actual}, expected {expected}"));
        }
    }
    let row = api.pixel()?;
    let palette = [0, 0xffffffff, 0x010305];
    let destination = [0xaabbccdd, 0xffffffff, 0xff030507];
    let expected = [0xaabbccdd, 0xfefefe, 0x010305, 0xdeadbeef];
    let mut output = [0, 0, 0, 0xdeadbeef];
    // All pointers refer to live, aligned buffers of the advertised lengths.
    let result = unsafe {
        row(
            [0, 1, 2].as_ptr(),
            3,
            palette.as_ptr(),
            3,
            destination.as_ptr(),
            1,
            1,
            output.as_mut_ptr(),
            4,
        )
    };
    if result != 0 || output != expected {
        return Err(format!(
            "pixel success smoke: status={result}, output={output:x?}"
        ));
    }
    // Invalid index is deliberately last: an implementation that writes early pixels fails.
    let result = unsafe {
        row(
            [1, 1, 3].as_ptr(),
            3,
            palette.as_ptr(),
            3,
            [0; 3].as_ptr(),
            0,
            0,
            output.as_mut_ptr(),
            4,
        )
    };
    if result != 201 || output != expected {
        return Err(format!(
            "pixel atomic failure smoke: status={result}, output={output:x?}"
        ));
    }
    Ok(())
}
fn argument(args: impl IntoIterator<Item = OsString>) -> Result<PathBuf, String> {
    let mut args = args.into_iter();
    let path = args
        .next()
        .ok_or("usage: ar-native-probe <absolute-dll-path>")?;
    if args.next().is_some() {
        return Err("expected exactly one DLL path".into());
    }
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err("DLL path must be absolute".into());
    }
    Ok(path)
}
// Bounds checked before LoadLibrary: reject malformed, non-x64 and non-DLL PE files.
fn check_pe(bytes: &[u8]) -> Result<(), String> {
    let invalid = || "expected an x64 PE DLL".to_string();
    if bytes.get(..2) != Some(b"MZ") {
        return Err(invalid());
    }
    let offset = u32::from_le_bytes(
        bytes
            .get(0x3c..0x40)
            .ok_or_else(invalid)?
            .try_into()
            .unwrap(),
    ) as usize;
    let end = offset.checked_add(24).ok_or_else(invalid)?;
    let header = bytes.get(offset..end).ok_or_else(invalid)?;
    if &header[..4] != b"PE\0\0"
        || u16::from_le_bytes([header[4], header[5]]) != 0x8664
        || u16::from_le_bytes([header[22], header[23]]) & 0x2000 == 0
    {
        return Err(invalid());
    }
    Ok(())
}
#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        ffi::{c_void, CString},
        os::windows::ffi::OsStrExt,
    };
    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryExW(path: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *const c_void;
        fn FreeLibrary(module: *mut c_void) -> i32;
    }
    struct Library(*mut c_void);
    impl Drop for Library {
        fn drop(&mut self) {
            unsafe {
                FreeLibrary(self.0);
            }
        }
    }
    impl Library {
        fn symbol(&self, name: &str) -> Result<*const c_void, String> {
            let name_c = CString::new(name).map_err(|e| e.to_string())?;
            let address = unsafe { GetProcAddress(self.0, name_c.as_ptr().cast()) };
            if address.is_null() {
                Err(format!("missing export {name}"))
            } else {
                Ok(address)
            }
        }
    }
    impl Api for Library {
        fn resolve(&self, name: &str) -> Result<(), String> {
            self.symbol(name).map(|_| ())
        }
        fn u32_query(&self, name: &str) -> Result<u32, String> {
            let f: unsafe extern "C" fn() -> u32 =
                unsafe { std::mem::transmute(self.symbol(name)?) };
            Ok(unsafe { f() })
        }
        fn size_query(&self, name: &str) -> Result<usize, String> {
            let f: unsafe extern "C" fn() -> usize =
                unsafe { std::mem::transmute(self.symbol(name)?) };
            Ok(unsafe { f() })
        }
        fn pixel(&self) -> Result<PixelRow, String> {
            Ok(unsafe { std::mem::transmute(self.symbol("ar_particle_pixel_row")?) })
        }
    }
    pub fn probe(path: &Path) -> Result<(), String> {
        if cfg!(not(target_arch = "x86_64")) {
            return Err("probe requires Windows x64".into());
        }
        check_pe(&std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?)?;
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err("DLL path contains NUL".into());
        }
        wide.push(0);
        // Search dependencies only beside the explicit DLL and in System32.
        let handle = unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), 0x100 | 0x800) };
        if handle.is_null() {
            return Err(format!(
                "LoadLibraryExW: {}",
                std::io::Error::last_os_error()
            ));
        }
        audit(&Library(handle))
    }
}
fn probe(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows::probe(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("DLL probe requires Windows x64".into())
    }
}
fn main() -> std::process::ExitCode {
    match argument(std::env::args_os().skip(1)).and_then(|path| probe(&path)) {
        Ok(()) => {
            println!("PASS: native DLL exports, ABI/sizes, pixel success and atomic failure");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("FAIL: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        missing: Option<&'static str>,
        wrong: Option<&'static str>,
        row: PixelRow,
    }
    impl Api for Fake {
        fn resolve(&self, name: &str) -> Result<(), String> {
            if self.missing == Some(name) {
                Err(format!("missing export {name}"))
            } else {
                Ok(())
            }
        }
        fn u32_query(&self, name: &str) -> Result<u32, String> {
            Ok(U32_QUERIES.iter().find(|x| x.0 == name).unwrap().1
                + u32::from(self.wrong == Some(name)))
        }
        fn size_query(&self, name: &str) -> Result<usize, String> {
            Ok(SIZE_QUERIES.iter().find(|x| x.0 == name).unwrap().1
                + usize::from(self.wrong == Some(name)))
        }
        fn pixel(&self) -> Result<PixelRow, String> {
            Ok(self.row)
        }
    }
    fn fake() -> Fake {
        Fake {
            missing: None,
            wrong: None,
            row: against_rome_data::particle_pixel_ffi::ar_particle_pixel_row,
        }
    }
    #[test]
    fn real_rust_pixel_contract_passes() {
        audit(&fake()).unwrap();
    }
    #[test]
    fn every_required_export_is_mandatory() {
        for &name in EXPORTS {
            let mut api = fake();
            api.missing = Some(name);
            assert!(audit(&api).unwrap_err().contains(name));
        }
    }
    #[test]
    fn every_abi_and_size_mismatch_is_rejected() {
        for name in U32_QUERIES
            .iter()
            .map(|x| x.0)
            .chain(SIZE_QUERIES.iter().map(|x| x.0))
        {
            let mut api = fake();
            api.wrong = Some(name);
            assert!(audit(&api).unwrap_err().contains(name));
        }
    }
    unsafe extern "C" fn bad_success(
        _: *const u8,
        _: usize,
        _: *const u32,
        _: usize,
        _: *const u32,
        _: u32,
        _: u32,
        _: *mut u32,
        _: usize,
    ) -> i32 {
        0
    }
    unsafe extern "C" fn partial_failure(
        i: *const u8,
        n: usize,
        p: *const u32,
        l: usize,
        d: *const u32,
        m: u32,
        z: u32,
        o: *mut u32,
        c: usize,
    ) -> i32 {
        let result = unsafe {
            against_rome_data::particle_pixel_ffi::ar_particle_pixel_row(i, n, p, l, d, m, z, o, c)
        };
        if result == 201 {
            unsafe {
                *o = 0;
            }
        }
        result
    }
    #[test]
    fn no_op_success_is_rejected() {
        let mut api = fake();
        api.row = bad_success;
        assert!(audit(&api).unwrap_err().contains("success smoke"));
    }
    #[test]
    fn partial_write_on_late_failure_is_rejected() {
        let mut api = fake();
        api.row = partial_failure;
        assert!(audit(&api).unwrap_err().contains("atomic failure"));
    }
    #[test]
    fn cli_requires_one_absolute_path() {
        assert!(argument([]).is_err());
        assert!(argument(["local.dll".into()]).is_err());
        let absolute = std::env::current_dir()
            .unwrap()
            .join("chosen DLL.dll")
            .into_os_string();
        assert!(argument([absolute.clone()]).is_ok());
        assert!(argument([absolute, "extra".into()]).is_err());
    }
    #[test]
    fn truncated_and_wrong_pe_headers_fail_closed() {
        let mut bytes = vec![0; 256];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&64u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
        bytes[86..88].copy_from_slice(&0x2000u16.to_le_bytes());
        check_pe(&bytes).unwrap();
        for n in 0..88 {
            assert!(check_pe(&bytes[..n]).is_err());
        }
        let mut bad = bytes.clone();
        bad[68] = 0;
        assert!(check_pe(&bad).is_err());
        let mut bad = bytes.clone();
        bad[86..88].fill(0);
        assert!(check_pe(&bad).is_err());
        bytes[0x3c..0x40].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(check_pe(&bytes).is_err());
    }
}
