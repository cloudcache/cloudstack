use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct CryptoService {
    cipher: Aes256Gcm,
}

impl CryptoService {
    /// `key_b64` is a base64-encoded 32-byte key from `QS_ENCRYPTION_KEY`.
    pub fn new(key_b64: &str) -> anyhow::Result<Self> {
        let raw = B64.decode(key_b64.trim())?;
        if raw.len() != 32 {
            anyhow::bail!("QS_ENCRYPTION_KEY must be 32 bytes (base64-encoded)");
        }
        let key = Key::<Aes256Gcm>::from_slice(&raw);
        Ok(Self {
            cipher: Aes256Gcm::new(key),
        })
    }

    /// Encrypt plaintext. Returns `base64(nonce || ciphertext)`.
    pub fn encrypt(&self, plaintext: &str) -> AppResult<String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| AppError::Crypto(e.to_string()))?;

        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(B64.encode(combined))
    }

    /// Decrypt value produced by `encrypt`.
    pub fn decrypt(&self, encoded: &str) -> AppResult<String> {
        let combined = B64
            .decode(encoded.trim())
            .map_err(|e| AppError::Crypto(format!("base64 decode: {e}")))?;

        if combined.len() < 12 {
            return Err(AppError::Crypto("ciphertext too short".into()));
        }

        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| AppError::Crypto(format!("decrypt: {e}")))?;

        String::from_utf8(plaintext).map_err(|e| AppError::Crypto(e.to_string()))
    }

    /// SHA-256 hex digest (used for JWT token_hash in user_sessions).
    pub fn sha256_hex(input: &str) -> String {
        let digest = Sha256::digest(input.as_bytes());
        hex::encode(digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as B64;

    fn test_service() -> CryptoService {
        let key = B64.encode([0u8; 32]);
        CryptoService::new(&key).unwrap()
    }

    #[test]
    fn roundtrip() {
        let svc = test_service();
        let original = "super-secret-password";
        let encrypted = svc.encrypt(original).unwrap();
        let decrypted = svc.decrypt(&encrypted).unwrap();
        assert_eq!(original, decrypted);
    }

    #[test]
    fn sha256_stable() {
        let h = CryptoService::sha256_hex("hello");
        assert_eq!(
            h,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}
