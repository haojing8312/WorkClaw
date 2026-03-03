use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use anyhow::{Result, anyhow};

const PBKDF2_ITERATIONS: u32 = 100_000;
const VERIFY_PLAINTEXT: &[u8] = b"SKILLMINT_OK";

pub fn derive_key(username: &str, skill_id: &str, skill_name: &str) -> [u8; 32] {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(skill_id.as_bytes());
    hasher.update(skill_name.as_bytes());
    let salt = hasher.finalize();

    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(
        username.as_bytes(),
        &salt,
        PBKDF2_ITERATIONS,
        &mut key,
    );
    key
}

pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| anyhow!("encrypt error: {e}"))?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    if data.len() < 12 {
        return Err(anyhow!("data too short"));
    }
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow!("decrypt failed — wrong username?"))
}

pub fn make_verify_token(key: &[u8; 32]) -> Result<String> {
    let encrypted = encrypt(VERIFY_PLAINTEXT, key)?;
    Ok(B64.encode(&encrypted))
}

pub fn check_verify_token(token: &str, key: &[u8; 32]) -> bool {
    let Ok(data) = B64.decode(token) else { return false };
    let Ok(plain) = decrypt(&data, key) else { return false };
    plain == VERIFY_PLAINTEXT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key_deterministic() {
        let k1 = derive_key("alice", "skill-id-123", "合同审查");
        let k2 = derive_key("alice", "skill-id-123", "合同审查");
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_derive_key_different_users() {
        let k1 = derive_key("alice", "skill-id-123", "合同审查");
        let k2 = derive_key("bob", "skill-id-123", "合同审查");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = derive_key("alice", "skill-id-123", "test");
        let plaintext = b"Hello, WorkClaw!";
        let encrypted = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = derive_key("alice", "skill-id-123", "test");
        let key2 = derive_key("bob", "skill-id-123", "test");
        let encrypted = encrypt(b"secret", &key1).unwrap();
        assert!(decrypt(&encrypted, &key2).is_err());
    }

    #[test]
    fn test_verify_token_roundtrip() {
        let key = derive_key("alice", "id", "name");
        let token = make_verify_token(&key).unwrap();
        assert!(check_verify_token(&token, &key));
    }

    #[test]
    fn test_verify_token_wrong_key() {
        let key1 = derive_key("alice", "id", "name");
        let key2 = derive_key("bob", "id", "name");
        let token = make_verify_token(&key1).unwrap();
        assert!(!check_verify_token(&token, &key2));
    }
}
