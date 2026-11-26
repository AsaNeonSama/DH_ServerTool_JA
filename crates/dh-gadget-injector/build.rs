use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use sha2::{Digest, Sha256};

const GADGET_VERSION: &str = "17.27.4";
const GADGET_XZ_FILENAME: &str = "frida-gadget-17.27.4-windows-x86_64.dll.xz";
const GADGET_ZST_FILENAME: &str = "frida-gadget-17.27.4-windows-x86_64.dll.zst";
const GADGET_MARKER_FILENAME: &str = "frida-gadget-17.27.4-windows-x86_64.dll.zst.sha256";
const GADGET_DOWNLOAD_URL: &str =
    "https://github.com/frida/frida/releases/download/17.27.4/frida-gadget-17.27.4-windows-x86_64.dll.xz";

const EXPECTED_XZ_SHA256: &str = "ccce545b7b7f5aeed837df95566f74720e7520f01d5995d96e75f5454501ace3";
const EXPECTED_DLL_SHA256: &str =
    "e79d94834bde95493411e7626aee5fe5f18e1e47ca39fe16f5d1660067e67da1";

type BuildResult<T> = std::result::Result<T, Box<dyn Error>>;

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

fn verify_zst_cache(zst_path: &Path, marker_path: &Path) -> bool {
    let Ok(meta): std::io::Result<fs::Metadata> = fs::metadata(zst_path) else {
        return false;
    };
    if meta.len() == 0 {
        return false;
    }

    let expected_marker: String = format!("{EXPECTED_DLL_SHA256}:{}", meta.len());
    if let Ok(marker) = fs::read_to_string(marker_path) {
        if marker.trim() == expected_marker {
            return true;
        }
    }

    // Marker missing or mismatched: verify by decompressing and hashing the DLL payload.
    let Ok(zst_bytes): std::io::Result<Vec<u8>> = fs::read(zst_path) else {
        return false;
    };
    let Ok(dll_bytes): std::io::Result<Vec<u8>> = zstd::stream::decode_all(&zst_bytes[..]) else {
        return false;
    };
    if sha256_hex(&dll_bytes) != EXPECTED_DLL_SHA256 {
        return false;
    }
    let _ = fs::write(marker_path, expected_marker);
    true
}

fn download_xz(dest_xz: &Path) -> BuildResult<()> {
    let tmp_xz: PathBuf = dest_xz.with_extension("xz.tmp");
    let status: ExitStatus = Command::new("curl.exe")
        .args(["-fsSL", "-o"])
        .arg(&tmp_xz)
        .arg(GADGET_DOWNLOAD_URL)
        .status()?;

    if !status.success() {
        let _ = fs::remove_file(&tmp_xz);
        return Err(format!(
            "curl.exe failed to download {GADGET_DOWNLOAD_URL} (exit: {status}). \
             You can also download it manually or set DH_FRIDA_GADGET_PATH."
        )
        .into());
    }

    let xz_bytes: Vec<u8> = match fs::read(&tmp_xz) {
        Ok(bytes) => bytes,
        Err(err) => {
            let _ = fs::remove_file(&tmp_xz);
            return Err(err.into());
        }
    };
    let actual_xz_hash: String = sha256_hex(&xz_bytes);
    if actual_xz_hash != EXPECTED_XZ_SHA256 {
        let _ = fs::remove_file(&tmp_xz);
        return Err(format!(
            "SHA-256 mismatch for downloaded {GADGET_XZ_FILENAME}: expected {EXPECTED_XZ_SHA256}, got {actual_xz_hash}"
        )
        .into());
    }

    if dest_xz.exists() {
        let _ = fs::remove_file(dest_xz);
    }
    fs::rename(&tmp_xz, dest_xz)?;
    Ok(())
}

fn decompress_xz_to_dll(xz_path: &Path) -> BuildResult<Vec<u8>> {
    let xz_bytes: Vec<u8> = fs::read(xz_path)?;
    let actual_xz_hash: String = sha256_hex(&xz_bytes);
    if actual_xz_hash != EXPECTED_XZ_SHA256 {
        return Err(format!(
            "SHA-256 mismatch for {}: expected {EXPECTED_XZ_SHA256}, got {actual_xz_hash}",
            xz_path.display()
        )
        .into());
    }

    let mut dll_bytes: Vec<u8> = Vec::with_capacity(36 * 1024 * 1024);
    let mut reader: BufReader<&[u8]> = BufReader::new(&xz_bytes[..]);
    lzma_rs::xz_decompress(&mut reader, &mut dll_bytes)?;
    Ok(dll_bytes)
}

fn compress_and_store_zst(
    dll_bytes: &[u8],
    zst_path: &Path,
    marker_path: &Path,
) -> BuildResult<()> {
    let actual_dll_hash: String = sha256_hex(dll_bytes);
    if actual_dll_hash != EXPECTED_DLL_SHA256 {
        return Err(format!(
            "SHA-256 mismatch for uncompressed frida-gadget DLL: expected {EXPECTED_DLL_SHA256}, got {actual_dll_hash}"
        )
        .into());
    }

    let zst_bytes: Vec<u8> = zstd::stream::encode_all(dll_bytes, 22)?;

    let tmp_zst: PathBuf = zst_path.with_extension("zst.tmp");
    fs::write(&tmp_zst, &zst_bytes)?;
    if zst_path.exists() {
        let _ = fs::remove_file(zst_path);
    }
    fs::rename(&tmp_zst, zst_path)?;

    let marker: String = format!("{EXPECTED_DLL_SHA256}:{}", zst_bytes.len());
    fs::write(marker_path, marker)?;
    Ok(())
}

fn main() -> BuildResult<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DH_FRIDA_GADGET_PATH");

    let manifest_dir: PathBuf = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    // tools/crates/dh-gadget-injector -> tools/.build-cache
    let cache_dir: PathBuf = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or("workspace root expected two levels above crate")?
        .join(".build-cache");

    fs::create_dir_all(&cache_dir)?;

    let zst_path: PathBuf = cache_dir.join(GADGET_ZST_FILENAME);
    let marker_path: PathBuf = cache_dir.join(GADGET_MARKER_FILENAME);
    let xz_path: PathBuf = cache_dir.join(GADGET_XZ_FILENAME);

    if !verify_zst_cache(&zst_path, &marker_path) {
        if let Ok(custom_path_str) = env::var("DH_FRIDA_GADGET_PATH") {
            let custom_path: PathBuf = PathBuf::from(custom_path_str);
            let ext: String = custom_path
                .extension()
                .and_then(|s: &std::ffi::OsStr| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            match ext.as_str() {
                "zst" => {
                    fs::copy(&custom_path, &zst_path)?;
                    if !verify_zst_cache(&zst_path, &marker_path) {
                        let _ = fs::remove_file(&zst_path);
                        return Err("DH_FRIDA_GADGET_PATH .zst failed SHA-256 verification".into());
                    }
                }
                "xz" => {
                    let dll_bytes: Vec<u8> = decompress_xz_to_dll(&custom_path)?;
                    compress_and_store_zst(&dll_bytes, &zst_path, &marker_path)?;
                }
                _ => {
                    let dll_bytes: Vec<u8> = fs::read(&custom_path)?;
                    compress_and_store_zst(&dll_bytes, &zst_path, &marker_path)?;
                }
            }
        } else {
            if !xz_path.exists() {
                download_xz(&xz_path)?;
            }
            let dll_bytes: Vec<u8> = decompress_xz_to_dll(&xz_path)?;
            compress_and_store_zst(&dll_bytes, &zst_path, &marker_path)?;
        }
    }

    println!(
        "cargo:rustc-env=DH_FRIDA_GADGET_ZST_PATH={}",
        zst_path.display()
    );
    println!("cargo:rustc-env=DH_FRIDA_GADGET_SHA256={EXPECTED_DLL_SHA256}");
    println!("cargo:rustc-env=DH_FRIDA_GADGET_VERSION={GADGET_VERSION}");
    Ok(())
}
