use google_cloud_auth::credentials::{AccessTokenCredentials, Builder};
use tokio::sync::OnceCell;
use tracing::{debug, info};

use crate::core::error::{AppError, AppResult};

/// Shared GCP token provider using Application Default Credentials (ADC).
///
/// Works transparently across environments:
/// - GKE with Workload Identity (metadata server)
/// - Compute Engine (default service account)
/// - Local dev (`GOOGLE_APPLICATION_CREDENTIALS` or `gcloud auth application-default login`)
///
/// Credentials are lazily initialized on first `token()` call, so construction
/// never fails. This allows test code to create instances without ADC configured
/// (as long as env-var token fallbacks are used in the calling service).
pub struct GcpTokenProvider {
    credentials: OnceCell<AccessTokenCredentials>,
}

impl Default for GcpTokenProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GcpTokenProvider {
    pub fn new() -> Self {
        info!("GCP token provider created (credentials will be initialized on first use)");
        Self {
            credentials: OnceCell::new(),
        }
    }

    async fn get_credentials(&self) -> AppResult<&AccessTokenCredentials> {
        self.credentials
            .get_or_try_init(|| async {
                Builder::default()
                    .with_scopes(["https://www.googleapis.com/auth/cloud-platform"])
                    .build_access_token_credentials()
                    .map_err(|e| {
                        AppError::InternalError(format!(
                            "Failed to initialize GCP credentials (ADC): {e}"
                        ))
                    })
            })
            .await
    }

    /// Get a valid OAuth2 access token, refreshing automatically when expired.
    pub async fn token(&self) -> AppResult<String> {
        let creds = self.get_credentials().await?;
        let access_token =
            tokio::time::timeout(std::time::Duration::from_secs(5), creds.access_token())
                .await
                .map_err(|_| AppError::InternalError("GCP token acquisition timed out".into()))?
                .map_err(|e| {
                    AppError::InternalError(format!("Failed to obtain GCP access token: {e}"))
                })?;
        debug!("Obtained GCP access token via ADC");
        Ok(access_token.token)
    }
}
