use argon2::{
    password_hash::{rand_core::OsRng, Error, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, PasswordHash,
};
use base64::Engine;
use ring::rand::SecureRandom;

pub struct TokenService;

impl TokenService {
    pub const fn new() -> Self {
        Self
    }

    pub fn hash_password(&self, pw: &str) -> Result<String, Error> {
        let salt = SaltString::generate(&mut OsRng);
        Ok(Argon2::default()
            .hash_password(pw.as_bytes(), &salt)?
            .to_string())
    }

    pub fn verify_password(&self, pw: &str, pw_hash: &str) -> Result<bool, Error> {
        let parsed_hash = PasswordHash::new(pw_hash)?;
        let result = Argon2::default().verify_password(pw.as_bytes(), &parsed_hash);

        match result {
            Ok(_) => Ok(true),
            Err(Error::Password) => Ok(false),
            Err(err) => Err(err),
        }
    }

    /// Generates a random base64 encoded secure token.
    /// The output length is always `252` bytes (characters).
    pub fn generate_token(&self) -> Result<String, ()> {
        const ENCODER: base64::engine::GeneralPurpose = base64::engine::GeneralPurpose::new(
            &base64::alphabet::URL_SAFE,
            base64::engine::GeneralPurposeConfig::new().with_encode_padding(true),
        );

        // floor(254 / 4) * 3 = 189
        let mut buf = [0u8; 189];
        let rng = ring::rand::SystemRandom::new();
        rng.fill(&mut buf).map_err(|_| ())?;

        Ok(ENCODER.encode(buf))
    }
}
