use std::env;
use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir_raw: String = env::var("CARGO_MANIFEST_DIR")?;
    let manifest_dir: PathBuf = PathBuf::from(manifest_dir_raw);
    let icon_path: PathBuf = manifest_dir.join("icons").join("icon.ico");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", icon_path.display());

    let win_attrs: tauri_build::WindowsAttributes =
        tauri_build::WindowsAttributes::new().window_icon_path(&icon_path);
    let attrs: tauri_build::Attributes =
        tauri_build::Attributes::new().windows_attributes(win_attrs);
    tauri_build::try_build(attrs)?;
    Ok(())
}
