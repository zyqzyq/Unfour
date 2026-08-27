use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};

pub(crate) struct PendingAuthorization {
    pub state: String,
    pub code_verifier: String,
    pub code_challenge: String,
}

impl PendingAuthorization {
    pub(crate) fn generate() -> Self {
        let state = random_url_safe(32);
        let code_verifier = random_url_safe(64);
        let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
        Self {
            state,
            code_verifier,
            code_challenge,
        }
    }
}

fn random_url_safe(byte_count: usize) -> String {
    let mut bytes = vec![0_u8; byte_count];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_values_are_url_safe_and_use_s256() {
        let pending = PendingAuthorization::generate();
        assert_eq!(pending.state.len(), 43);
        assert_eq!(pending.code_verifier.len(), 86);
        assert_eq!(pending.code_challenge.len(), 43);
        for value in [
            &pending.state,
            &pending.code_verifier,
            &pending.code_challenge,
        ] {
            assert!(value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'));
        }
        assert_eq!(
            pending.code_challenge,
            URL_SAFE_NO_PAD.encode(Sha256::digest(pending.code_verifier.as_bytes()))
        );
    }

    #[test]
    fn each_attempt_has_fresh_state_and_verifier() {
        let first = PendingAuthorization::generate();
        let second = PendingAuthorization::generate();
        assert_ne!(first.state, second.state);
        assert_ne!(first.code_verifier, second.code_verifier);
    }
}
