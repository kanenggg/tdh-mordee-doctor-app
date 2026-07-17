use std::convert::TryFrom;

use pasetors::claims::{Claims, ClaimsValidationRules};
use pasetors::keys::SymmetricKey;
use pasetors::token::UntrustedToken;
use pasetors::{local, version4::V4, Local};
use serde::{de::DeserializeOwned, Serialize};

use crate::core::error::{AppError, AppResult};

/// Encrypts and decrypts sensitive summarization fields using Paseto v4 Local
/// (XChaCha20-Poly1305 symmetric encryption).
///
/// The key is loaded from `config.paseto.local_key` (32-byte hex string) and must
/// be overridden via `PASETO__LOCAL_KEY` env var in production.
pub struct SummarizationEncryptor {
    key: SymmetricKey<V4>,
}

impl SummarizationEncryptor {
    /// Construct from a 64-character hex-encoded 32-byte key.
    pub fn from_hex(hex_key: &str) -> AppResult<Self> {
        let bytes = hex::decode(hex_key)
            .map_err(|e| AppError::InternalError(format!("Invalid paseto key hex: {e}")))?;
        if bytes.len() != 32 {
            return Err(AppError::InternalError(format!(
                "Paseto local key must be 32 bytes, got {}",
                bytes.len()
            )));
        }
        let key = SymmetricKey::<V4>::from(&bytes)
            .map_err(|e| AppError::InternalError(format!("Failed to build paseto key: {e}")))?;
        Ok(Self { key })
    }

    /// Serialize `value` to JSON then encrypt as a Paseto v4 Local token string.
    pub fn encrypt<T: Serialize>(&self, value: &T) -> AppResult<String> {
        let json = serde_json::to_string(value)
            .map_err(|e| AppError::InternalError(format!("Serialization error: {e}")))?;

        let mut claims = Claims::new()
            .map_err(|e| AppError::InternalError(format!("Failed to create claims: {e}")))?;
        // Stored data has no expiry — disable the default 1-hour exp.
        claims.non_expiring();
        claims
            .add_additional("payload", json)
            .map_err(|e| AppError::InternalError(format!("Failed to add claim: {e}")))?;

        local::encrypt(&self.key, &claims, None, None)
            .map_err(|e| AppError::InternalError(format!("Paseto encryption error: {e}")))
    }

    /// Decrypt a Paseto v4 Local token string and deserialize the payload as `T`.
    pub fn decrypt<T: DeserializeOwned>(&self, token: &str) -> AppResult<T> {
        let mut rules = ClaimsValidationRules::new();
        rules.allow_non_expiring();

        let untrusted = UntrustedToken::<Local, V4>::try_from(token)
            .map_err(|e| AppError::InternalError(format!("Invalid paseto token: {e}")))?;

        let trusted = local::decrypt(&self.key, &untrusted, &rules, None, None)
            .map_err(|e| AppError::InternalError(format!("Paseto decryption error: {e}")))?;

        let json = trusted
            .payload_claims()
            .and_then(|c| c.get_claim("payload"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::InternalError("Missing payload claim in token".to_string()))?;

        serde_json::from_str(json)
            .map_err(|e| AppError::InternalError(format!("Deserialization error: {e}")))
    }
}
