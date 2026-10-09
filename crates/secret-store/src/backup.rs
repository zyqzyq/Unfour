//! Password-protected local backup envelopes. Never used by Cloud Sync.
use base64::{engine::general_purpose::STANDARD, Engine};
use ring::{
    aead, pbkdf2,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use unfour_core::{AppError, AppResult};
use zeroize::Zeroizing;

pub const MAX_ARCHIVE_BYTES: usize = 48 * 1024 * 1024;
const MAX_PLAIN_BYTES: usize = 32 * 1024 * 1024;
const ITERATIONS: u32 = 600_000;
const FORMAT: &str = "unfour-workspace-encrypted";
const AAD: &[u8] = b"unfour-workspace-encrypted:1:PBKDF2-SHA256:600000:AES-256-GCM";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    format: String,
    version: u32,
    salt: String,
    key_nonce: String,
    wrapped_key: String,
    nonce: String,
    ciphertext: String,
}

fn error() -> AppError {
    AppError::Validation("WORKSPACE_BUNDLE_UNLOCK_FAILED".into())
}
fn random<const N: usize>() -> AppResult<[u8; N]> {
    let mut bytes = [0; N];
    SystemRandom::new().fill(&mut bytes).map_err(|_| error())?;
    Ok(bytes)
}
fn derive(password: &str, salt: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0; 32]);
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(ITERATIONS).unwrap(),
        salt,
        password.as_bytes(),
        key.as_mut(),
    );
    key
}
fn seal(key: &[u8], nonce: [u8; 12], bytes: &mut Vec<u8>) -> AppResult<()> {
    aead::LessSafeKey::new(aead::UnboundKey::new(&aead::AES_256_GCM, key).map_err(|_| error())?)
        .seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(AAD),
            bytes,
        )
        .map_err(|_| error())
}
fn open(key: &[u8], nonce: [u8; 12], bytes: &mut Vec<u8>) -> AppResult<()> {
    let length = aead::LessSafeKey::new(
        aead::UnboundKey::new(&aead::AES_256_GCM, key).map_err(|_| error())?,
    )
    .open_in_place(
        aead::Nonce::assume_unique_for_key(nonce),
        aead::Aad::from(AAD),
        bytes,
    )
    .map_err(|_| error())?
    .len();
    bytes.truncate(length);
    Ok(())
}
fn decode<const N: usize>(text: &str) -> AppResult<[u8; N]> {
    STANDARD
        .decode(text)
        .map_err(|_| error())?
        .try_into()
        .map_err(|_| error())
}
pub fn is_encrypted(content: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(content)
        .ok()
        .is_some_and(|value| value["format"] == FORMAT)
}
pub fn encrypt(plaintext: &[u8], password: &str) -> AppResult<String> {
    if password.chars().count() < 12 || password.len() > 1024 {
        return Err(AppError::Validation(
            "WORKSPACE_BUNDLE_PASSWORD_LENGTH".into(),
        ));
    }
    if plaintext.len() > MAX_PLAIN_BYTES {
        return Err(error());
    }
    let salt = random::<16>()?;
    let key_nonce = random::<12>()?;
    let nonce = random::<12>()?;
    let key = Zeroizing::new(random::<32>()?);
    let wrapping_key = derive(password, &salt);
    let mut wrapped_key = Zeroizing::new(key.to_vec());
    seal(wrapping_key.as_ref(), key_nonce, &mut wrapped_key)?;
    let mut ciphertext = Zeroizing::new(plaintext.to_vec());
    seal(key.as_ref(), nonce, &mut ciphertext)?;
    serde_json::to_string(&Envelope {
        format: FORMAT.into(),
        version: 1,
        salt: STANDARD.encode(salt),
        key_nonce: STANDARD.encode(key_nonce),
        wrapped_key: STANDARD.encode(&*wrapped_key),
        nonce: STANDARD.encode(nonce),
        ciphertext: STANDARD.encode(&*ciphertext),
    })
    .map_err(|_| error())
}
pub fn decrypt(content: &str, password: &str) -> AppResult<Zeroizing<Vec<u8>>> {
    if content.len() > MAX_ARCHIVE_BYTES || password.len() > 1024 {
        return Err(error());
    }
    let envelope: Envelope = serde_json::from_str(content).map_err(|_| error())?;
    if envelope.format != FORMAT || envelope.version != 1 {
        return Err(error());
    }
    // Fixed versioned parameters prevent attacker-controlled KDF resource exhaustion.
    let salt = decode::<16>(&envelope.salt)?;
    let key_nonce = decode::<12>(&envelope.key_nonce)?;
    let nonce = decode::<12>(&envelope.nonce)?;
    let mut key = Zeroizing::new(
        STANDARD
            .decode(&envelope.wrapped_key)
            .map_err(|_| error())?,
    );
    if key.len() != 48 {
        return Err(error());
    }
    let wrapping_key = derive(password, &salt);
    open(wrapping_key.as_ref(), key_nonce, &mut key)?;
    let mut plaintext = Zeroizing::new(STANDARD.decode(&envelope.ciphertext).map_err(|_| error())?);
    if plaintext.len() > MAX_PLAIN_BYTES + 16 {
        return Err(error());
    }
    open(&key, nonce, &mut plaintext)?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_password_tamper_and_randomization() {
        let password = "disposable-backup-password";
        let archive = encrypt(b"secret-canary", password).unwrap();
        assert!(!archive.contains("secret-canary"));
        assert_ne!(archive, encrypt(b"secret-canary", password).unwrap());
        assert_eq!(&**decrypt(&archive, password).unwrap(), b"secret-canary");
        assert!(decrypt(&archive, "wrong-password").is_err());
        let mut envelope: Envelope = serde_json::from_str(&archive).unwrap();
        let mut bytes = STANDARD.decode(&envelope.ciphertext).unwrap();
        bytes[0] ^= 1;
        envelope.ciphertext = STANDARD.encode(bytes);
        assert!(decrypt(&serde_json::to_string(&envelope).unwrap(), password).is_err());
        assert!(encrypt(b"x", "short").is_err());
    }
}
