use std::env;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;

const CIPHER_INLINE_TOKEN: &str = "/*__DH_CIPHER_INLINE__*/";

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir_raw: String = env::var("CARGO_MANIFEST_DIR")?;
    let manifest_dir: PathBuf = PathBuf::from(manifest_dir_raw);
    let out_dir_raw: String = env::var("OUT_DIR")?;
    let out_dir: PathBuf = PathBuf::from(out_dir_raw);

    let assets_dir: PathBuf = manifest_dir.join("..").join("..").join("assets");
    let console_dir: PathBuf = assets_dir.join("console");

    let icon_path: PathBuf = assets_dir.join("icon.ico");
    let dh_cipher_path: PathBuf = console_dir.join("dh_cipher.js");
    let dh_console_path: PathBuf = console_dir.join("dh_console.html");
    let un_auth_path: PathBuf = console_dir.join("un_auth.html");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", icon_path.display());
    println!("cargo:rerun-if-changed={}", dh_cipher_path.display());
    println!("cargo:rerun-if-changed={}", dh_console_path.display());
    println!("cargo:rerun-if-changed={}", un_auth_path.display());

    let dh_cipher_js: String = fs::read_to_string(&dh_cipher_path)?;
    let dh_console_template: String = fs::read_to_string(&dh_console_path)?;
    let un_auth_html: String = fs::read_to_string(&un_auth_path)?;

    assert!(
        dh_console_template.contains(CIPHER_INLINE_TOKEN),
        "Placeholder {CIPHER_INLINE_TOKEN} not found in {}",
        dh_console_path.display()
    );

    let dh_console_html: String =
        dh_console_template.replacen(CIPHER_INLINE_TOKEN, &dh_cipher_js, 1);

    write_precompressed_html(&out_dir, "dh_console", dh_console_html.as_bytes())?;
    write_precompressed_html(&out_dir, "un_auth", un_auth_html.as_bytes())?;

    embed_windows_resources(&icon_path)?;
    Ok(())
}

fn write_precompressed_html(out_dir: &Path, stem: &str, raw: &[u8]) -> Result<(), Box<dyn Error>> {
    let raw_path: PathBuf = out_dir.join(format!("{stem}.raw.html"));
    fs::write(&raw_path, raw)?;

    let mut gz_encoder: GzEncoder<Vec<u8>> = GzEncoder::new(Vec::new(), Compression::new(6));
    gz_encoder.write_all(raw)?;
    let gz_bytes: Vec<u8> = gz_encoder.finish()?;
    let gz_path: PathBuf = out_dir.join(format!("{stem}.html.gz"));
    fs::write(&gz_path, gz_bytes)?;

    let mut br_bytes: Vec<u8> = Vec::new();
    {
        let mut br_writer: brotli::CompressorWriter<&mut Vec<u8>> =
            brotli::CompressorWriter::new(&mut br_bytes, 4096, 6, 22);
        br_writer.write_all(raw)?;
        br_writer.flush()?;
    }
    let br_path: PathBuf = out_dir.join(format!("{stem}.html.br"));
    fs::write(&br_path, br_bytes)?;
    Ok(())
}

#[cfg(windows)]
fn embed_windows_resources(icon_path: &Path) -> Result<(), Box<dyn Error>> {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return Ok(());
    }
    let Some(icon_str): Option<&str> = icon_path.to_str() else {
        return Err("icon.ico path must be valid UTF-8".into());
    };
    let mut res: winresource::WindowsResource = winresource::WindowsResource::new();
    res.set_icon(icon_str);
    res.set("ProductName", "DH_ServerTool_JA");
    res.set(
        "FileDescription",
        "Dread Hunger Dedicated Server Controller & Web Console",
    );
    res.set("CompanyName", "アサネオン / Build With Antigravity");
    res.set("LegalCopyright", "アサネオン / Build With Antigravity");
    res.set("OriginalFilename", "DH_ServerTool_JA.exe");
    res.set("InternalName", "DH_ServerTool_JA");
    res.set("FileVersion", "2.0.0.0");
    res.set("ProductVersion", "2.0.0.0");
    res.compile()?;
    Ok(())
}

#[cfg(not(windows))]
fn embed_windows_resources(_icon_path: &Path) -> Result<(), Box<dyn Error>> {
    Ok(())
}
