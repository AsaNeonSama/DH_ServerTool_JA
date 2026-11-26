use aes_gcm::aead::AeadInPlace;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce, Tag};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use subtle::ConstantTimeEq;

pub use crate::consts::crypto::{
    AES_AUTH_TAG_BYTES, AES_IV_BYTES, AES_KEY_BYTES, AUTH_CHALLENGE_BYTES, AUTH_CHALLENGE_TTL_MS,
    AUTH_PATH, AUTH_PSK_SEPARATOR, AUTH_VERIFY_PATH, COOKIE_AAD, MAX_PENDING_CHALLENGES,
};
pub use crate::structs::crypto::AesGcmSessionCipher;
use crate::structs::crypto::AesNonce;

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
    pub fn psk(&self) -> &str {
        &self.psk
    }

    #[must_use]
    pub fn encrypt(&self, plaintext: &str, aad: Option<&str>) -> String {
        let mut iv: [u8; AES_IV_BYTES] = [0u8; AES_IV_BYTES];
        OsRng.fill_bytes(&mut iv);
        self.encrypt_with_iv(plaintext, aad, iv)
    }

    #[must_use]
    pub fn encrypt_with_iv(
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
