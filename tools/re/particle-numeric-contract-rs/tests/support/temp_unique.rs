use std::{fs, path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}, time::{SystemTime, UNIX_EPOCH}};

pub fn under(base: &Path, label: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fs::create_dir_all(base).expect("建立測試根目錄");
    loop {
        let path = base.join(format!("{label}-{}-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).expect("取得測試時間").as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)));
        match fs::create_dir(&path) {
            Ok(()) => return path,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("建立測試目錄 {}: {error}", path.display()),
        }
    }
}

#[allow(dead_code)]
pub fn dir(label: &str) -> PathBuf {
    under(&std::env::temp_dir(), label)
}