use aes_gcm::aead::{AeadCore, AeadInPlace};
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce, Tag};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use subtle::ConstantTimeEq;

type AesNonce = Nonce<<Aes256Gcm as AeadCore>::NonceSize>;

pub const AES_KEY_BYTES: usize = 32;
pub const AES_IV_BYTES: usize = 12;
pub const AES_AUTH_TAG_BYTES: usize = 16;
pub const AUTH_PSK_SEPARATOR: &str = "#";
pub const COOKIE_AAD: &str = "COOKIE:dh_console_auth";
pub const AUTH_CHALLENGE_BYTES: usize = 16;
pub const AUTH_CHALLENGE_TTL_MS: u64 = 60_000;
pub const MAX_PENDING_CHALLENGES: usize = 256;
pub const AUTH_PATH: &str = "/dh-console/auth";
pub const AUTH_VERIFY_PATH: &str = "/dh-console/auth/verify";

#[must_use]
pub fn timing_safe_str_eq(a: &str, b: &str) -> bool {
    let bytes_a: &[u8] = a.as_bytes();
    let bytes_b: &[u8] = b.as_bytes();
    bytes_a.len() == bytes_b.len() && bool::from(bytes_a.ct_eq(bytes_b))
}

#[must_use]
pub fn encode_b64url_nopad(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Strictly decodes canonical `base64url` (no padding). Rejects empty inputs,
/// non-canonical trailing bits, padding `=`, or non-`base64url` characters.
#[must_use]
pub fn decode_canonical_b64url(encoded: &str) -> Option<Vec<u8>> {
    if encoded.is_empty() {
        return None;
    }
    let decoded: Vec<u8> = URL_SAFE_NO_PAD.decode(encoded).ok()?;
    if URL_SAFE_NO_PAD.encode(&decoded) != encoded {
        return None;
    }
    Some(decoded)
}

#[must_use]
pub fn random_b64url_bytes(len: usize) -> String {
    let mut buf: Vec<u8> = vec![0u8; len];
    OsRng.fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(&buf)
}

/// AES-256-GCM session cipher with byte-exact wire compatibility with
/// `assets/console/dh_cipher.js`:
/// `base64url_nopad( IV[12] || AuthTag[16] || Ciphertext[N] )`
#[derive(Clone)]
pub struct AesGcmSessionCipher {
    cipher: Aes256Gcm,
    psk: String,
}

impl AesGcmSessionCipher {
    #[must_use]
    pub fn generate() -> Self {
        let mut key: [u8; AES_KEY_BYTES] = [0u8; AES_KEY_BYTES];
        OsRng.fill_bytes(&mut key);
        Self::from_raw_key(key)
    }

    #[must_use]
    pub fn from_raw_key(raw_key: [u8; AES_KEY_BYTES]) -> Self {
        let key: &Key<Aes256Gcm> = Key::<Aes256Gcm>::from_slice(&raw_key);
        let cipher: Aes256Gcm = Aes256Gcm::new(key);
        let psk: String = URL_SAFE_NO_PAD.encode(raw_key);
        Self { cipher, psk }
    }

    #[must_use]
    pub fn from_psk(psk: &str) -> Option<Self> {
        let decoded: Vec<u8> = decode_canonical_b64url(psk)?;
        let raw_key: [u8; AES_KEY_BYTES] = decoded.try_into().ok()?;
        Some(Self::from_raw_key(raw_key))
    }

    #[must_use]
    pub fn psk(&self) -> &str {
        &self.psk
    }

    #[must_use]
    pub fn encrypt(&self, plaintext: &str, aad: Option<&str>) -> String {
        let mut iv: [u8; AES_IV_BYTES] = [0u8; AES_IV_BYTES];
        OsRng.fill_bytes(&mut iv);
        self.encrypt_with_iv(plaintext, aad, iv)
    }

    fn encrypt_with_iv(
        &self,
        plaintext: &str,
        aad: Option<&str>,
        iv: [u8; AES_IV_BYTES],
    ) -> String {
        let nonce: &AesNonce = Nonce::from_slice(&iv);
        let aad_bytes: &[u8] = aad.map_or(&[][..], str::as_bytes);
        let mut ciphertext: Vec<u8> = plaintext.as_bytes().to_vec();
        let tag: Tag =
            match self
                .cipher
                .encrypt_in_place_detached(nonce, aad_bytes, &mut ciphertext)
            {
                Ok(t) => t,
                Err(_) => {
                    unreachable!("AES-256-GCM encryption cannot fail for valid UTF-8 message")
                }
            };

        let mut packed: Vec<u8> =
            Vec::with_capacity(AES_IV_BYTES + AES_AUTH_TAG_BYTES + ciphertext.len());
        packed.extend_from_slice(&iv);
        packed.extend_from_slice(tag.as_slice());
        packed.extend_from_slice(&ciphertext);
        URL_SAFE_NO_PAD.encode(&packed)
    }

    #[must_use]
    pub fn decrypt(&self, encoded: &str, aad: Option<&str>) -> Option<String> {
        let raw: Vec<u8> = decode_canonical_b64url(encoded)?;
        let min_len: usize = AES_IV_BYTES + AES_AUTH_TAG_BYTES;
        if raw.len() < min_len {
            return None;
        }

        let iv: &[u8] = &raw[..AES_IV_BYTES];
        let tag_bytes: &[u8] = &raw[AES_IV_BYTES..min_len];
        let mut buffer: Vec<u8> = raw[min_len..].to_vec();

        let nonce: &AesNonce = Nonce::from_slice(iv);
        let tag: &Tag = Tag::from_slice(tag_bytes);
        let aad_bytes: &[u8] = aad.map_or(&[][..], str::as_bytes);

        self.cipher
            .decrypt_in_place_detached(nonce, aad_bytes, &mut buffer, tag)
            .ok()?;

        String::from_utf8(buffer).ok()
    }

    #[must_use]
    pub fn verify(&self, encoded: &str, expected_plaintext: &str, aad: Option<&str>) -> bool {
        self.decrypt(encoded, aad)
            .is_some_and(|decrypted: String| timing_safe_str_eq(&decrypted, expected_plaintext))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::Command;

    #[test]
    fn roundtrip_and_aad_binding() {
        let cipher = AesGcmSessionCipher::generate();
        assert_eq!(decode_canonical_b64url(cipher.psk()).unwrap().len(), 32);

        let aad = Some("REQ:/dh-console/console_command");
        let plaintext = r#"{"player_name":"Captain","command":"summon BP_Prey_Rat_C"}"#;
        let enc = cipher.encrypt(plaintext, aad);

        assert_eq!(cipher.decrypt(&enc, aad).as_deref(), Some(plaintext));
        assert!(cipher.verify(&enc, plaintext, aad));
        assert!(!cipher.verify(&enc, "wrong", aad));

        // Wrong AAD must fail
        assert_eq!(
            cipher.decrypt(&enc, Some("RES:/dh-console/console_command")),
            None
        );
        assert_eq!(cipher.decrypt(&enc, None), None);
    }

    #[test]
    fn handles_empty_plaintext_and_unicode() {
        let cipher = AesGcmSessionCipher::generate();

        let empty_enc = cipher.encrypt("", Some(COOKIE_AAD));
        assert_eq!(decode_canonical_b64url(&empty_enc).unwrap().len(), 28);
        assert_eq!(
            cipher.decrypt(&empty_enc, Some(COOKIE_AAD)).as_deref(),
            Some("")
        );

        let jp = "船長 が大爆裂魔法・チャージ 120 %を召喚しました🚀";
        let jp_enc = cipher.encrypt(jp, Some("RES:/dh-console/send_thrall_msg"));
        assert_eq!(
            cipher
                .decrypt(&jp_enc, Some("RES:/dh-console/send_thrall_msg"))
                .as_deref(),
            Some(jp)
        );
    }

    #[test]
    fn rejects_tampered_and_non_canonical_inputs() {
        let cipher = AesGcmSessionCipher::from_raw_key([0x42u8; 32]);
        let enc = cipher.encrypt("hello", Some("AAD"));
        let mut raw = decode_canonical_b64url(&enc).unwrap();

        // Tamper IV
        raw[0] ^= 0x01;
        assert_eq!(
            cipher.decrypt(&encode_b64url_nopad(&raw), Some("AAD")),
            None
        );
        raw[0] ^= 0x01;

        // Tamper AuthTag (bytes 12..28)
        raw[15] ^= 0x80;
        assert_eq!(
            cipher.decrypt(&encode_b64url_nopad(&raw), Some("AAD")),
            None
        );
        raw[15] ^= 0x80;

        // Tamper Ciphertext (bytes 28..)
        raw[28] ^= 0x01;
        assert_eq!(
            cipher.decrypt(&encode_b64url_nopad(&raw), Some("AAD")),
            None
        );

        // Truncated (< 28 bytes)
        assert_eq!(
            cipher.decrypt(&encode_b64url_nopad(&raw[..27]), Some("AAD")),
            None
        );

        // Empty / padded / dirty base64url
        assert_eq!(cipher.decrypt("", Some("AAD")), None);
        assert_eq!(cipher.decrypt(&format!("{enc}=="), Some("AAD")), None);
        assert_eq!(cipher.decrypt(&format!("{enc} "), Some("AAD")), None);

        // Non-canonical trailing bits (e.g. 1 byte encoded with non-zero unused lower bits)
        assert_eq!(decode_canonical_b64url("AB"), None);
    }

    #[test]
    fn wire_layout_matches_iv_tag_ciphertext() {
        let key = [0x11u8; 32];
        let iv = [0x22u8; 12];
        let cipher = AesGcmSessionCipher::from_raw_key(key);
        let encoded = cipher.encrypt_with_iv("test", Some("REQ:/x"), iv);
        let raw = decode_canonical_b64url(&encoded).unwrap();
        assert_eq!(&raw[..12], &iv);
        assert_eq!(raw.len(), 12 + 16 + 4);
    }

    #[test]
    fn cross_language_interop_with_dh_cipher_js_via_bun() {
        let dh_cipher_js_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("assets")
            .join("console")
            .join("dh_cipher.js");
        assert!(
            dh_cipher_js_path.exists(),
            "dh_cipher.js not found at {}",
            dh_cipher_js_path.display()
        );

        let cipher = AesGcmSessionCipher::generate();
        let psk = cipher.psk();

        let rust_enc_req = cipher.encrypt(
            r#"{"player_name":"Captain","msg":"こんにちは! ⚓"}"#,
            Some("REQ:/dh-console/send_thrall_msg"),
        );
        let rust_enc_empty = cipher.encrypt("", Some("RES:/dh-console/auth/verify"));
        let rust_enc_no_aad = cipher.encrypt("plain-no-aad", None);

        let script = r#"
const fs = require("fs");
const input = JSON.parse(process.env.DH_INTEROP_PAYLOAD);
const { cipherPath, psk, rustEncReq, rustEncEmpty, rustEncNoAad } = input;
const code = fs.readFileSync(cipherPath, "utf8");
new Function(code)();
const c = globalThis.__dhCipher;
if (!c || !c.isValidPsk(psk)) {
  console.error("Invalid PSK in dh_cipher.js");
  process.exit(1);
}

const decReq = c.decrypt(rustEncReq, psk, "REQ:/dh-console/send_thrall_msg");
const decWrongAad = c.decrypt(rustEncReq, psk, "RES:/dh-console/send_thrall_msg");
const decEmpty = c.decrypt(rustEncEmpty, psk, "RES:/dh-console/auth/verify");
const decNoAad = c.decrypt(rustEncNoAad, psk, "");

// Tamper tag byte inside base64url payload
const badChar = rustEncReq[20] === "A" ? "B" : "A";
const tampered = rustEncReq.slice(0, 20) + badChar + rustEncReq.slice(21);
const decTampered = c.decrypt(tampered, psk, "REQ:/dh-console/send_thrall_msg");

// Malformed base64url (padding / non-canonical / too short)
const decPadded = c.decrypt(rustEncReq + "=", psk, "REQ:/dh-console/send_thrall_msg");
const decShort = c.decrypt("AAAA", psk, "REQ:/dh-console/send_thrall_msg");

const jsEncReq = c.encrypt(JSON.stringify({ ok: true, msg: "日本語テスト" }), psk, "REQ:/dh-console/add_item");
const jsEncCookie = c.encrypt("550E8400-E29B-41D4-A716-446655440000", psk, "COOKIE:dh_console_auth");
const jsEncEmpty = c.encrypt("", psk, "RES:/dh-console/empty");

console.log(JSON.stringify({
  decReq,
  decWrongAad,
  decEmpty,
  decNoAad,
  decTampered,
  decPadded,
  decShort,
  jsEncReq,
  jsEncCookie,
  jsEncEmpty
}));
"#;

        let payload = serde_json::json!({
            "cipherPath": dh_cipher_js_path.to_string_lossy(),
            "psk": psk,
            "rustEncReq": rust_enc_req,
            "rustEncEmpty": rust_enc_empty,
            "rustEncNoAad": rust_enc_no_aad,
        });

        let output = Command::new("bun")
            .arg("-e")
            .arg(script)
            .env("DH_INTEROP_PAYLOAD", payload.to_string())
            .output()
            .expect("bun must be available to run cross-language AES-GCM test");

        assert!(
            output.status.success(),
            "bun interop script failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
        let v: serde_json::Value =
            serde_json::from_str(stdout.trim()).expect("bun output must be JSON");

        // Verify JS decrypted Rust's ciphertexts accurately
        assert_eq!(
            v["decReq"].as_str(),
            Some(r#"{"player_name":"Captain","msg":"こんにちは! ⚓"}"#)
        );
        assert!(v["decWrongAad"].is_null());
        assert_eq!(v["decEmpty"].as_str(), Some(""));
        assert_eq!(v["decNoAad"].as_str(), Some("plain-no-aad"));
        assert!(v["decTampered"].is_null());
        assert!(v["decPadded"].is_null());
        assert!(v["decShort"].is_null());

        // Verify Rust decrypts JS's ciphertexts accurately
        let js_enc_req = v["jsEncReq"].as_str().unwrap();
        let js_enc_cookie = v["jsEncCookie"].as_str().unwrap();
        let js_enc_empty = v["jsEncEmpty"].as_str().unwrap();

        assert_eq!(
            cipher
                .decrypt(js_enc_req, Some("REQ:/dh-console/add_item"))
                .as_deref(),
            Some(r#"{"ok":true,"msg":"日本語テスト"}"#)
        );
        assert_eq!(
            cipher.decrypt(js_enc_req, Some("RES:/dh-console/add_item")),
            None
        );
        assert!(cipher.verify(
            js_enc_cookie,
            "550E8400-E29B-41D4-A716-446655440000",
            Some(COOKIE_AAD)
        ));
        assert_eq!(
            cipher
                .decrypt(js_enc_empty, Some("RES:/dh-console/empty"))
                .as_deref(),
            Some("")
        );
    }
}
