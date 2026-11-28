use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn format_usize_literal(value: usize) -> String {
    let digits: String = value.to_string();
    let mut out: String = String::with_capacity(digits.len() + digits.len() / 3);
    for (idx, ch) in digits.chars().enumerate() {
        let remaining: usize = digits.len() - idx;
        if idx > 0 && remaining.is_multiple_of(3) {
            out.push('_');
        }
        out.push(ch);
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher: Sha256 = Sha256::new();
    hasher.update(bytes);
    let digest: [u8; 32] = hasher.finalize().into();
    let mut out: String = String::with_capacity(64);
    for b in digest {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn compress_server_dll(
    src_path: &Path,
    out_dir: &Path,
    out_file_name: &str,
) -> Result<(String, usize), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={}", src_path.display());

    let raw_bytes: Vec<u8> = fs::read(src_path).map_err(|err: std::io::Error| {
        format!(
            "Failed to read staged server voice DLL at {}: {err}. Build dh-server-voice first.",
            src_path.display()
        )
    })?;
    if raw_bytes.len() < 2 || raw_bytes[0] != b'M' || raw_bytes[1] != b'Z' {
        return Err(format!(
            "Staged server voice DLL {} is not a valid PE binary",
            src_path.display()
        )
        .into());
    }

    let sha256: String = sha256_hex(&raw_bytes);
    let raw_len: usize = raw_bytes.len();
    let compressed: Vec<u8> = zstd::stream::encode_all(&raw_bytes[..], 19)?;
    let out_path: PathBuf = out_dir.join(out_file_name);
    fs::write(&out_path, compressed)?;
    Ok((sha256, raw_len))
}

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir_raw: String = env::var("CARGO_MANIFEST_DIR")?;
    let manifest_dir: PathBuf = PathBuf::from(manifest_dir_raw);
    let out_dir_raw: String = env::var("OUT_DIR")?;
    let out_dir: PathBuf = PathBuf::from(out_dir_raw);

    let icon_path: PathBuf = manifest_dir.join("icons").join("icon.ico");
    let voice_server_dll_src: PathBuf = manifest_dir
        .join("..")
        .join("..")
        .join("assets")
        .join("voice_chat")
        .join("server")
        .join("dh_voice_server.dll");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed={}", icon_path.display());

    let (voice_sha, voice_len): (String, usize) =
        compress_server_dll(&voice_server_dll_src, &out_dir, "dh_voice_server.dll.zst")?;
    let voice_len_lit: String = format_usize_literal(voice_len);

    let meta_rs: String = format!(
        "pub const VOICE_SERVER_DLL_SHA256: &str = \"{voice_sha}\";\n\
         pub const VOICE_SERVER_DLL_RAW_LEN: usize = {voice_len_lit};\n"
    );
    fs::write(out_dir.join("builtin_server_dlls.rs"), meta_rs)?;

    let win_attrs: tauri_build::WindowsAttributes =
        tauri_build::WindowsAttributes::new().window_icon_path(&icon_path);
    let attrs: tauri_build::Attributes =
        tauri_build::Attributes::new().windows_attributes(win_attrs);
    tauri_build::try_build(attrs)?;
    Ok(())
}
