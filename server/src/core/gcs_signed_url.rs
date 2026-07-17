use std::sync::Arc;

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use google_cloud_auth::{credentials::Builder, signer::Signer};
use jiff::Timestamp;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::OnceCell;

use crate::core::error::{AppError, AppResult};
use crate::core::gcp_auth::GcpTokenProvider;
use crate::syst::config::GcsConfig;

const HOST: &str = "storage.googleapis.com";
const CLOUD_STORAGE_HOST: &str = "storage.cloud.google.com";
const ALGORITHM: &str = "GOOG4-RSA-SHA256";
const SIGNED_HEADERS: &str = "host";
const MAX_TTL_SECS: u32 = 604_800;
const PATH_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}');
const QUERY_ENCODE_SET: &AsciiSet = &PATH_ENCODE_SET
    .add(b'!')
    .add(b'$')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'=')
    .add(b'@')
    .add(b'[')
    .add(b']');

#[async_trait]
pub trait GcsSignedUrlGenerator: Send + Sync {
    async fn generate_signed_url(&self, source_url: &str) -> AppResult<String>;
}

#[async_trait]
pub trait BlobSigner: Send + Sync {
    async fn client_email(&self) -> AppResult<String>;
    async fn sign_blob(&self, payload: &str) -> AppResult<Vec<u8>>;
}

pub struct GcpGcsSignedUrlGenerator {
    bucket_name: String,
    base_url: String,
    signing_service_account_email: String,
    ttl_secs: u32,
    signer: Arc<dyn BlobSigner>,
}

struct GcsObject {
    bucket: String,
    object: String,
}

impl GcpGcsSignedUrlGenerator {
    pub fn new(config: &GcsConfig, token: Arc<GcpTokenProvider>) -> Self {
        let signing_service_account_email = config.signing_service_account_email.clone();
        let signer: Arc<dyn BlobSigner> = if signing_service_account_email.trim().is_empty() {
            Arc::new(AdcBlobSigner::new())
        } else {
            Arc::new(IamBlobSigner::new(
                token,
                signing_service_account_email.clone(),
            ))
        };
        Self::with_signer(config, signer)
    }

    pub fn with_signer(config: &GcsConfig, signer: Arc<dyn BlobSigner>) -> Self {
        Self {
            bucket_name: config.bucket_name.trim_matches('/').to_string(),
            base_url: config.base_url.trim_end_matches('/').to_string(),
            signing_service_account_email: config.signing_service_account_email.clone(),
            ttl_secs: config.signed_url_ttl_secs,
            signer,
        }
    }

    fn ensure_configured(&self) -> AppResult<()> {
        if self.ttl_secs == 0 || self.ttl_secs > MAX_TTL_SECS {
            return Err(AppError::InternalError(format!(
                "GCS signed_url_ttl_secs must be between 1 and {MAX_TTL_SECS}"
            )));
        }
        Ok(())
    }

    async fn signing_service_account_email(&self) -> AppResult<String> {
        let configured = self.signing_service_account_email.trim();
        if !configured.is_empty() {
            return Ok(configured.to_string());
        }

        let email = self.signer.client_email().await?;
        let email = email.trim();
        if email.is_empty() {
            return Err(AppError::InternalError(
                "GCS signing service account email resolved to empty value".into(),
            ));
        }
        Ok(email.to_string())
    }

    async fn generate_for_object(
        &self,
        bucket_name: &str,
        object: &str,
        now: Timestamp,
    ) -> AppResult<String> {
        self.ensure_configured()?;
        if bucket_name.is_empty() {
            return Err(AppError::InternalError(
                "GCS bucket_name is not configured".into(),
            ));
        }
        let signing_service_account_email = self.signing_service_account_email().await?;

        let date = now.strftime("%Y%m%d").to_string();
        let timestamp = now.strftime("%Y%m%dT%H%M%SZ").to_string();
        let credential_scope = format!("{date}/auto/storage/goog4_request");
        let credential = format!("{signing_service_account_email}/{credential_scope}");
        let canonical_uri = format!(
            "/{}/{}",
            path_encode(bucket_name),
            object
                .split('/')
                .map(path_encode)
                .collect::<Vec<_>>()
                .join("/")
        );

        let canonical_query_string = canonical_query_string(&[
            ("X-Goog-Algorithm", ALGORITHM.to_string()),
            ("X-Goog-Credential", credential.clone()),
            ("X-Goog-Date", timestamp.clone()),
            ("X-Goog-Expires", self.ttl_secs.to_string()),
            ("X-Goog-SignedHeaders", SIGNED_HEADERS.to_string()),
        ]);
        let canonical_request = format!(
            "GET\n{canonical_uri}\n{canonical_query_string}\nhost:{HOST}\n\n{SIGNED_HEADERS}\nUNSIGNED-PAYLOAD"
        );
        let canonical_request_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
        let string_to_sign =
            format!("{ALGORITHM}\n{timestamp}\n{credential_scope}\n{canonical_request_hash}");
        let signature = hex::encode(self.signer.sign_blob(&string_to_sign).await?);

        Ok(format!(
            "https://{HOST}{canonical_uri}?{canonical_query_string}&X-Goog-Signature={signature}"
        ))
    }
}

#[async_trait]
impl GcsSignedUrlGenerator for GcpGcsSignedUrlGenerator {
    async fn generate_signed_url(&self, source_url: &str) -> AppResult<String> {
        let parsed = parse_gcs_object(source_url, &self.bucket_name, &self.base_url)?;
        self.generate_for_object(&parsed.bucket, &parsed.object, Timestamp::now())
            .await
    }
}

pub struct IamBlobSigner {
    http: reqwest::Client,
    token: Arc<GcpTokenProvider>,
    service_account_email: String,
}

impl IamBlobSigner {
    pub fn new(token: Arc<GcpTokenProvider>, service_account_email: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            token,
            service_account_email,
        }
    }
}

#[derive(Serialize)]
struct SignBlobRequest {
    payload: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignBlobResponse {
    signed_blob: String,
}

#[async_trait]
impl BlobSigner for IamBlobSigner {
    async fn client_email(&self) -> AppResult<String> {
        let email = self.service_account_email.trim();
        if email.is_empty() {
            return Err(AppError::InternalError(
                "GCS signing_service_account_email is not configured".into(),
            ));
        }
        Ok(email.to_string())
    }

    async fn sign_blob(&self, payload: &str) -> AppResult<Vec<u8>> {
        if self.service_account_email.trim().is_empty() {
            return Err(AppError::InternalError(
                "GCS signing_service_account_email is not configured".into(),
            ));
        }

        let token = self.token.token().await?;
        let url = format!(
            "https://iamcredentials.googleapis.com/v1/projects/-/serviceAccounts/{}:signBlob",
            path_encode(&self.service_account_email)
        );
        let resp = self
            .http
            .post(url)
            .bearer_auth(token)
            .json(&SignBlobRequest {
                payload: STANDARD.encode(payload.as_bytes()),
            })
            .send()
            .await
            .map_err(|e| AppError::UpstreamError(format!("iam signBlob request: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::UpstreamError(format!("iam signBlob status: {e}")))?
            .json::<SignBlobResponse>()
            .await
            .map_err(|e| AppError::UpstreamError(format!("iam signBlob decode: {e}")))?;

        STANDARD
            .decode(resp.signed_blob)
            .map_err(|e| AppError::UpstreamError(format!("iam signBlob base64: {e}")))
    }
}

pub struct AdcBlobSigner {
    signer: OnceCell<Signer>,
}

impl AdcBlobSigner {
    pub fn new() -> Self {
        Self {
            signer: OnceCell::new(),
        }
    }

    async fn signer(&self) -> AppResult<&Signer> {
        self.signer
            .get_or_try_init(|| async {
                Builder::default()
                    .build_signer()
                    .map_err(|e| AppError::InternalError(format!("GCS ADC signer init: {e}")))
            })
            .await
    }
}

impl Default for AdcBlobSigner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BlobSigner for AdcBlobSigner {
    async fn client_email(&self) -> AppResult<String> {
        self.signer()
            .await?
            .client_email()
            .await
            .map_err(|e| AppError::InternalError(format!("GCS ADC signer client email: {e}")))
    }

    async fn sign_blob(&self, payload: &str) -> AppResult<Vec<u8>> {
        self.signer()
            .await?
            .sign(payload.as_bytes())
            .await
            .map(|signature| signature.to_vec())
            .map_err(|e| AppError::UpstreamError(format!("GCS ADC signer signBlob: {e}")))
    }
}

fn parse_gcs_object(source_url: &str, bucket_name: &str, base_url: &str) -> AppResult<GcsObject> {
    let source = source_url.trim();
    if source.is_empty() {
        return Err(AppError::UpstreamError(
            "eKYC image URL is empty".to_string(),
        ));
    }

    let parsed = if let Some(rest) = source.strip_prefix("gs://") {
        parse_bucket_prefixed(rest)?
    } else if let Some(rest) = source.strip_prefix("https://storage.googleapis.com/") {
        parse_bucket_prefixed(rest)?
    } else if let Some(rest) = source.strip_prefix(&format!("https://{CLOUD_STORAGE_HOST}/")) {
        parse_bucket_prefixed(rest)?
    } else if let Some(rest) = source.strip_prefix(&format!("{}/", base_url.trim_end_matches('/')))
    {
        GcsObject {
            bucket: bucket_name.to_string(),
            object: strip_query_and_fragment(rest).to_string(),
        }
    } else if source.starts_with("http://") || source.starts_with("https://") {
        return Err(AppError::UpstreamError(format!(
            "unsupported eKYC image URL host: {source}"
        )));
    } else {
        GcsObject {
            bucket: bucket_name.to_string(),
            object: strip_query_and_fragment(source)
                .trim_start_matches('/')
                .to_string(),
        }
    };

    let object = percent_decode_str(parsed.object.trim_start_matches('/'))
        .decode_utf8()
        .map_err(|e| AppError::UpstreamError(format!("invalid eKYC image URL encoding: {e}")))?
        .to_string();

    if object.is_empty() {
        return Err(AppError::UpstreamError(
            "eKYC image object path is empty".to_string(),
        ));
    }

    Ok(GcsObject {
        bucket: parsed.bucket,
        object,
    })
}

fn parse_bucket_prefixed(path: &str) -> AppResult<GcsObject> {
    let path = strip_query_and_fragment(path);
    let (bucket, object) = path.split_once('/').ok_or_else(|| {
        AppError::UpstreamError("eKYC image URL does not contain an object path".to_string())
    })?;

    Ok(GcsObject {
        bucket: bucket.to_string(),
        object: object.to_string(),
    })
}

fn strip_query_and_fragment(value: &str) -> &str {
    value
        .split(['?', '#'])
        .next()
        .expect("split always returns one item")
}

fn canonical_query_string(params: &[(&str, String)]) -> String {
    let mut encoded = params
        .iter()
        .map(|(key, value)| (query_encode(key), query_encode(value)))
        .collect::<Vec<_>>();
    encoded.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    encoded
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn path_encode(value: &str) -> String {
    utf8_percent_encode(value, PATH_ENCODE_SET).to_string()
}

fn query_encode(value: &str) -> String {
    utf8_percent_encode(value, QUERY_ENCODE_SET).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedSigner;
    struct AnyPayloadSigner;

    #[async_trait]
    impl BlobSigner for FixedSigner {
        async fn client_email(&self) -> AppResult<String> {
            Ok("signer@example.iam.gserviceaccount.com".to_string())
        }

        async fn sign_blob(&self, payload: &str) -> AppResult<Vec<u8>> {
            assert!(payload.starts_with(
                "GOOG4-RSA-SHA256\n20260706T011530Z\n20260706/auto/storage/goog4_request\n"
            ));
            Ok(vec![0xab, 0xcd])
        }
    }

    #[async_trait]
    impl BlobSigner for AnyPayloadSigner {
        async fn client_email(&self) -> AppResult<String> {
            Ok("signer@example.iam.gserviceaccount.com".to_string())
        }

        async fn sign_blob(&self, _payload: &str) -> AppResult<Vec<u8>> {
            Ok(vec![0xab, 0xcd])
        }
    }

    fn config() -> GcsConfig {
        GcsConfig {
            bucket_name: "private-bucket".to_string(),
            base_url: "https://storage.googleapis.com/private-bucket".to_string(),
            signing_service_account_email: "signer@example.iam.gserviceaccount.com".to_string(),
            signed_url_ttl_secs: 300,
        }
    }

    fn config_without_bucket() -> GcsConfig {
        GcsConfig {
            bucket_name: String::new(),
            base_url: String::new(),
            signing_service_account_email: "signer@example.iam.gserviceaccount.com".to_string(),
            signed_url_ttl_secs: 300,
        }
    }

    fn config_without_signing_email() -> GcsConfig {
        GcsConfig {
            bucket_name: "private-bucket".to_string(),
            base_url: "https://storage.googleapis.com/private-bucket".to_string(),
            signing_service_account_email: String::new(),
            signed_url_ttl_secs: 300,
        }
    }

    fn assert_parsed(source_url: &str, expected_bucket: &str, expected_object: &str) {
        let parsed = parse_gcs_object(
            source_url,
            "private-bucket",
            "https://storage.googleapis.com/private-bucket",
        )
        .unwrap();

        assert_eq!(parsed.bucket, expected_bucket);
        assert_eq!(parsed.object, expected_object);
    }

    #[test]
    fn parses_supported_gcs_references() {
        let bucket = "private-bucket";
        let base = "https://storage.googleapis.com/private-bucket";

        assert_parsed("gs://private-bucket/a/b.png", "private-bucket", "a/b.png");
        assert_parsed(
            "https://storage.googleapis.com/private-bucket/a%20b.png",
            "private-bucket",
            "a b.png",
        );
        assert_parsed(
            "https://storage.googleapis.com/private-bucket/a/b.png?download=1",
            "private-bucket",
            "a/b.png",
        );
        assert_parsed(
            "https://storage.cloud.google.com/private-bucket/ekyc/626/card.jpg",
            "private-bucket",
            "ekyc/626/card.jpg",
        );
        assert_eq!(
            parse_gcs_object("a/b.png", bucket, base).unwrap().object,
            "a/b.png"
        );
    }

    #[test]
    fn rejects_unsupported_gcs_references() {
        let bucket = "private-bucket";
        let base = "https://storage.googleapis.com/private-bucket";

        assert!(parse_gcs_object("", bucket, base).is_err());
        assert!(parse_gcs_object("https://example.com/a.png", bucket, base).is_err());
    }

    #[tokio::test]
    async fn generates_v4_signed_get_url() {
        let generator = GcpGcsSignedUrlGenerator::with_signer(&config(), Arc::new(FixedSigner));
        let now = "2026-07-06T01:15:30Z".parse::<Timestamp>().unwrap();

        let url = generator
            .generate_for_object("private-bucket", "folder/id card.png", now)
            .await
            .unwrap();

        assert!(
            url.starts_with("https://storage.googleapis.com/private-bucket/folder/id%20card.png?")
        );
        assert!(url.contains("X-Goog-Algorithm=GOOG4-RSA-SHA256"));
        assert!(url.contains("X-Goog-Credential=signer%40example.iam.gserviceaccount.com%2F20260706%2Fauto%2Fstorage%2Fgoog4_request"));
        assert!(url.contains("X-Goog-Date=20260706T011530Z"));
        assert!(url.contains("X-Goog-Expires=300"));
        assert!(url.contains("X-Goog-SignedHeaders=host"));
        assert!(url.ends_with("X-Goog-Signature=abcd"));
    }

    #[tokio::test]
    async fn generates_signed_url_with_bucket_from_source_url() {
        let generator =
            GcpGcsSignedUrlGenerator::with_signer(&config(), Arc::new(AnyPayloadSigner));

        let url = generator
            .generate_signed_url("https://storage.cloud.google.com/source-bucket/ekyc/626/card.jpg")
            .await
            .unwrap();

        assert!(url.starts_with("https://storage.googleapis.com/source-bucket/ekyc/626/card.jpg?"));
    }

    #[tokio::test]
    async fn generates_signed_url_with_bucket_from_source_url_when_env_bucket_is_empty() {
        let generator = GcpGcsSignedUrlGenerator::with_signer(
            &config_without_bucket(),
            Arc::new(AnyPayloadSigner),
        );

        let url = generator
            .generate_signed_url("https://storage.cloud.google.com/source-bucket/ekyc/626/card.jpg")
            .await
            .unwrap();

        assert!(url.starts_with("https://storage.googleapis.com/source-bucket/ekyc/626/card.jpg?"));
    }

    #[tokio::test]
    async fn derives_signing_email_from_signer_when_config_is_empty() {
        let generator = GcpGcsSignedUrlGenerator::with_signer(
            &config_without_signing_email(),
            Arc::new(AnyPayloadSigner),
        );

        let url = generator
            .generate_signed_url("gs://private-bucket/ekyc/626/card.jpg")
            .await
            .unwrap();

        assert!(url.contains("X-Goog-Credential=signer%40example.iam.gserviceaccount.com%2F"));
    }
}
