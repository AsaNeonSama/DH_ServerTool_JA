use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use dh_client_launcher::discovery::{auto_detect_client_exe, parse_steam_library_folders_vdf};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        let seq: u64 = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let dir: PathBuf = std::env::temp_dir().join(format!(
            "dh_client_launcher_disc_{prefix}_{}_{}",
            std::process::id(),
            seq
        ));
        let _ = fs::remove_dir_all(&dir);
        assert!(
            fs::create_dir_all(&dir).is_ok(),
            "create temp dir: {}",
            dir.display()
        );
        Self { path: dir }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn touch(path: &Path) {
    if let Some(parent) = path.parent() {
        assert!(
            fs::create_dir_all(parent).is_ok(),
            "create parent dirs: {}",
            parent.display()
        );
    }
    assert!(
        fs::write(path, b"// stub").is_ok(),
        "write stub: {}",
        path.display()
    );
}

#[test]
fn parse_steam_library_folders_vdf_extracts_escaped_paths() {
    let vdf: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
	"2"
	{
		"path"		"D:\\SteamLibrary"
	}
}
"#;
    let roots: Vec<PathBuf> = parse_steam_library_folders_vdf(vdf);
    assert_eq!(
        roots,
        vec![
            PathBuf::from(r"C:\Program Files (x86)\Steam"),
            PathBuf::from(r"D:\SteamLibrary"),
        ]
    );
}

#[test]
fn auto_detect_client_exe_finds_relative_executable_first() {
    let tmp: TempDirGuard = TempDirGuard::new("detect_exe");
    touch(
        &tmp.path()
            .join("DreadHunger/Binaries/Win64/DreadHunger-Win64-Shipping.exe"),
    );

    let res = auto_detect_client_exe(tmp.path(), false);
    assert_eq!(
        res.found.as_deref(),
        Some("./DreadHunger/Binaries/Win64/DreadHunger-Win64-Shipping.exe")
    );
    assert_eq!(res.checked_count, 1);

    let empty_tmp: TempDirGuard = TempDirGuard::new("detect_exe_empty");
    let miss = auto_detect_client_exe(empty_tmp.path(), false);
    assert!(miss.found.is_none());
    assert!(miss.checked_count >= 1);
}
