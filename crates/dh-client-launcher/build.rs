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

fn compress_builtin_dll(
    src_path: &Path,
    out_dir: &Path,
    out_file_name: &str,
) -> Result<(String, usize), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={}", src_path.display());

    let raw_bytes: Vec<u8> = fs::read(src_path).map_err(|err: std::io::Error| {
        format!(
            "Failed to read staged built-in client DLL at {}: {err}. \
             Build dh-client-eos-fix (default + private-uri) and dh-client-voice first.",
            src_path.display()
        )
    })?;
    if raw_bytes.len() < 2 || raw_bytes[0] != b'M' || raw_bytes[1] != b'Z' {
        return Err(format!(
            "Staged built-in client DLL {} is not a valid PE binary",
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
    let assets_dir: PathBuf = manifest_dir.join("..").join("..").join("assets");

    let standard_eos_src: PathBuf = assets_dir
        .join("client_fix")
        .join("eos_fix")
        .join("plan_a")
        .join("eos_fix.dll");
    let private_uri_eos_src: PathBuf = assets_dir
        .join("client_fix")
        .join("eos_fix")
        .join("plan_b")
        .join("eos_fix.dll");
    let voice_src: PathBuf = assets_dir
        .join("voice_chat")
        .join("client")
        .join("dh_voice_client.dll");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed={}", icon_path.display());

    let (standard_sha, standard_len): (String, usize) =
        compress_builtin_dll(&standard_eos_src, &out_dir, "eos_fix_plan_a.dll.zst")?;
    let (private_sha, private_len): (String, usize) =
        compress_builtin_dll(&private_uri_eos_src, &out_dir, "eos_fix_plan_b.dll.zst")?;
    let (voice_sha, voice_len): (String, usize) =
        compress_builtin_dll(&voice_src, &out_dir, "dh_voice_client.dll.zst")?;

    let standard_len_lit: String = format_usize_literal(standard_len);
    let private_len_lit: String = format_usize_literal(private_len);
    let voice_len_lit: String = format_usize_literal(voice_len);

    let meta_rs: String = format!(
        "pub const EOS_FIX_PLAN_A_SHA256: &str = \"{standard_sha}\";\n\
         pub const EOS_FIX_PLAN_A_RAW_LEN: usize = {standard_len_lit};\n\
         pub const EOS_FIX_PLAN_B_SHA256: &str = \"{private_sha}\";\n\
         pub const EOS_FIX_PLAN_B_RAW_LEN: usize = {private_len_lit};\n\
         pub const VOICE_CLIENT_SHA256: &str = \"{voice_sha}\";\n\
         pub const VOICE_CLIENT_RAW_LEN: usize = {voice_len_lit};\n"
    );
    fs::write(out_dir.join("builtin_dlls_meta.rs"), meta_rs)?;

    let win_attrs: tauri_build::WindowsAttributes =
        tauri_build::WindowsAttributes::new().window_icon_path(&icon_path);
    let attrs: tauri_build::Attributes =
        tauri_build::Attributes::new().windows_attributes(win_attrs);
    tauri_build::try_build(attrs)?;
    Ok(())
}
