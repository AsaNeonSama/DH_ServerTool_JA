use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_resources()?;
    }

    Ok(())
}

#[cfg(windows)]
fn embed_windows_resources() -> Result<(), Box<dyn Error>> {
    let mut res: winresource::WindowsResource = winresource::WindowsResource::new();
    res.set("ProductName", "DH_Tool_JA");
    res.set(
        "FileDescription",
        "Dread Hunger Dedicated Server Zero-Extra-Port Voice Relay",
    );
    res.set("CompanyName", "アサネオン / Build With Antigravity");
    res.set("LegalCopyright", "アサネオン / Build With Antigravity");
    res.set("OriginalFilename", "dh_voice_server.dll");
    res.set("InternalName", "dh_voice_server");
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
