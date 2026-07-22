use base64::{Engine, engine::general_purpose::STANDARD};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};

#[derive(Clone)]
pub struct CredentialCipher([u8; 32]);

impl CredentialCipher {
    pub fn from_env() -> Result<Option<Self>, String> {
        let Ok(raw) = std::env::var("FILATURE_CREDENTIALS_KEY") else {
            return Ok(None);
        };
        let bytes = STANDARD
            .decode(raw.trim())
            .map_err(|_| "FILATURE_CREDENTIALS_KEY must be base64".to_string())?;
        if bytes.len() != 32 {
            return Err("FILATURE_CREDENTIALS_KEY must decode to exactly 32 bytes".into());
        }
        let mut key = [0_u8; 32];
        key.copy_from_slice(&bytes);
        Ok(Some(Self(key)))
    }
    pub fn encrypt(&self, clear: &str) -> Result<String, String> {
        let mut nonce = [0_u8; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| "credential nonce generation failed".to_string())?;
        let key = aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::AES_256_GCM, &self.0)
                .map_err(|_| "invalid credentials key".to_string())?,
        );
        let mut encrypted = clear.as_bytes().to_vec();
        key.seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::empty(),
            &mut encrypted,
        )
        .map_err(|_| "credential encryption failed".to_string())?;
        let mut packed = nonce.to_vec();
        packed.extend(encrypted);
        Ok(STANDARD.encode(packed))
    }
    pub fn decrypt(&self, packed: &str) -> Result<String, String> {
        let packed = STANDARD
            .decode(packed)
            .map_err(|_| "stored credential is not valid base64".to_string())?;
        if packed.len() < 13 {
            return Err("stored credential is truncated".into());
        }
        let mut nonce = [0_u8; 12];
        nonce.copy_from_slice(&packed[..12]);
        let mut encrypted = packed[12..].to_vec();
        let key = aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::AES_256_GCM, &self.0)
                .map_err(|_| "invalid credentials key".to_string())?,
        );
        let clear = key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::empty(),
                &mut encrypted,
            )
            .map_err(|_| {
                "FILATURE_CREDENTIALS_KEY cannot decrypt stored machine credentials".to_string()
            })?;
        String::from_utf8(clear.to_vec()).map_err(|_| "stored credential is not UTF-8".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_and_random_nonce() {
        let c = CredentialCipher([7; 32]);
        let a = c.encrypt("secret").unwrap();
        let b = c.encrypt("secret").unwrap();
        assert_ne!(a, b);
        assert_eq!(c.decrypt(&a).unwrap(), "secret");
        assert!(!a.contains("secret"));
    }
}
