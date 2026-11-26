use aes_gcm::aead::AeadCore;
use aes_gcm::{Aes256Gcm, Nonce};

pub(crate) type AesNonce = Nonce<<Aes256Gcm as AeadCore>::NonceSize>;

/// AES-256-GCM session cipher with byte-exact wire compatibility with
/// `assets/console/dh_cipher.js`:
/// `base64url_nopad( IV[12] || AuthTag[16] || Ciphertext[N] )`
#[derive(Clone)]
pub struct AesGcmSessionCipher {
    pub(crate) cipher: Aes256Gcm,
    pub(crate) psk: String,
}
