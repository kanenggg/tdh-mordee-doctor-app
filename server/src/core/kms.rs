use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::error::{AppError, AppResult};
use crate::core::gcp_auth::GcpTokenProvider;

#[async_trait]
pub trait KmsClient: Send + Sync {
    async fn encrypt(&self, plaintext: &str) -> AppResult<String>;
    async fn decrypt(&self, ciphertext_b64: &str) -> AppResult<String>;
    fn key_name(&self) -> &str;
}

pub struct GcpKmsGateway {
    http: reqwest::Client,
    token: Arc<GcpTokenProvider>,
    key_name: String,
}

impl GcpKmsGateway {
    pub fn new(token: Arc<GcpTokenProvider>, key_name: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            token,
            key_name,
        }
    }

    fn ensure_configured(&self) -> AppResult<()> {
        if self.key_name.is_empty() {
            return Err(AppError::InternalError(
                "KMS key_name is not configured (set kms.key_name / KMS__KEY_NAME)".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct EncryptReq {
    plaintext: String,
}
#[derive(Deserialize)]
struct EncryptResp {
    ciphertext: String,
}
#[derive(Serialize)]
struct DecryptReq {
    ciphertext: String,
}
#[derive(Deserialize)]
struct DecryptResp {
    plaintext: String,
}

#[async_trait]
impl KmsClient for GcpKmsGateway {
    async fn encrypt(&self, plaintext: &str) -> AppResult<String> {
        self.ensure_configured()?;
        let token = self.token.token().await?;
        let url = format!(
            "https://cloudkms.googleapis.com/v1/{}:encrypt",
            self.key_name
        );
        let resp = self
            .http
            .post(url)
            .bearer_auth(token)
            .json(&EncryptReq {
                plaintext: STANDARD.encode(plaintext),
            })
            .send()
            .await
            .map_err(|e| AppError::InternalError(format!("kms encrypt request: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::InternalError(format!("kms encrypt status: {e}")))?
            .json::<EncryptResp>()
            .await
            .map_err(|e| AppError::InternalError(format!("kms encrypt decode: {e}")))?;
        // KMS already returns base64; store as-is.
        Ok(resp.ciphertext)
    }

    async fn decrypt(&self, ciphertext_b64: &str) -> AppResult<String> {
        self.ensure_configured()?;
        let token = self.token.token().await?;
        let url = format!(
            "https://cloudkms.googleapis.com/v1/{}:decrypt",
            self.key_name
        );
        let resp = self
            .http
            .post(url)
            .bearer_auth(token)
            .json(&DecryptReq {
                ciphertext: ciphertext_b64.to_string(),
            })
            .send()
            .await
            .map_err(|e| AppError::InternalError(format!("kms decrypt request: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::InternalError(format!("kms decrypt status: {e}")))?
            .json::<DecryptResp>()
            .await
            .map_err(|e| AppError::InternalError(format!("kms decrypt decode: {e}")))?;
        let bytes = STANDARD
            .decode(resp.plaintext)
            .map_err(|e| AppError::InternalError(format!("kms decrypt base64: {e}")))?;
        String::from_utf8(bytes)
            .map_err(|e| AppError::InternalError(format!("kms decrypt utf8: {e}")))
    }

    fn key_name(&self) -> &str {
        &self.key_name
    }
}
