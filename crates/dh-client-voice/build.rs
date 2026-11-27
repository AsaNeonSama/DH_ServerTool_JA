use std::env;
use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir_raw: String = env::var("CARGO_MANIFEST_DIR")?;
    let manifest_dir: PathBuf = PathBuf::from(manifest_dir_raw);
    let def_path: PathBuf = manifest_dir.join("x3daudio1_7.def");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", def_path.display());

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let Some(def_str): Option<&str> = def_path.to_str() else {
            return Err("x3daudio1_7.def path must be valid UTF-8".into());
        };
        println!("cargo:rustc-cdylib-link-arg=/DEF:{def_str}");
        embed_windows_resources()?;
    }

    Ok(())
}

#[cfg(windows)]
fn embed_windows_resources() -> Result<(), Box<dyn Error>> {
    let mut res: winresource::WindowsResource = winresource::WindowsResource::new();
    res.set("ProductName", "DH_ServerTool_JA");
    res.set(
        "FileDescription",
        "Dread Hunger Client Dual-Mode UE4 Native Voice Bridge",
    );
    res.set("CompanyName", "アサネオン / Build With Antigravity");
    res.set("LegalCopyright", "アサネオン / Build With Antigravity");
    res.set("OriginalFilename", "dh_voice_client.dll");
    res.set("InternalName", "dh_voice_client");
    let pkg_version: &str = env!("CARGO_PKG_VERSION");
    let pe_version: String = format!("{pkg_version}.0");
    res.set("FileVersion", &pe_version);
    res.set("ProductVersion", &pe_version);
    res.compile()?;
    Ok(())
}

#[cfg(not(windows))]
fn embed_windows_resources() -> Result<(), Box<dyn Error>> {
    Ok(())
}
