//! Client for the payment service
//! `GET /payment/internal/transactions/{paymentTransactionId}`.
//!
//! Models the full `selectedChannelResult` tagged union so the mapper
//! can switch on its variants without re-parsing JSON. See spec:
//! docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::time::Duration;
use tracing::{debug, warn};

use crate::core::error::{AppError, AppResult};

#[derive(Debug)]
pub enum PaymentLookup {
    Found(PaymentDetail),
    NotFound,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentDetail {
    pub payment_transaction_id: i64,
    pub payment_transaction_ref_id: String,
    pub amount: serde_json::Number,
    /// May be null only on zero-amount free flows.
    #[serde(default)]
    pub selected_channel_result: Option<SelectedChannelResult>,
    /// Opaque pass-through; only `campaignName` is read by the mapper.
    #[serde(default)]
    pub coupon_protocol: Option<JsonValue>,
}

/// Top-level discriminator for which payment channel(s) covered the
/// transaction. The mapper MUST dispatch on the outer variant first
/// (SelfPay/Coverage/CoverageAndSelfPay) to determine `has_insurance`,
/// and only then look at the inner `PaymentChannel` for the display
/// name. Inverting that order would misclassify a Coverage payment
/// whose inner channel happens to be `PaymentChannel::Unknown`.
#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
pub enum SelectedChannelResult {
    #[serde(rename = "SelectedChannelResult.SelfPayChannel")]
    SelfPay { channel: PaymentChannel },
    #[serde(rename = "SelectedChannelResult.CoverageChannel")]
    Coverage { channel: PaymentChannel },
    #[serde(rename = "SelectedChannelResult.CoverageAndSelfPayChannel")]
    CoverageAndSelfPay {
        #[serde(rename = "coverageChannel")]
        coverage_channel: PaymentChannel,
        #[allow(dead_code)]
        #[serde(rename = "selfPayChannel")]
        self_pay_channel: PaymentChannel,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
pub enum PaymentChannel {
    // Self-pay variants — we only need to know they're self-pay.
    #[serde(rename = "PaymentChannelResult.PromptPay")]
    PromptPay,
    #[serde(rename = "PaymentChannelResult.TrueMoney")]
    TrueMoney,
    #[serde(rename = "PaymentChannelResult.Card")]
    Card,
    #[serde(rename = "PaymentChannelResult.CardSchedule")]
    CardSchedule,

    // Coverage — insurance v1/v2 share these fields.
    #[serde(rename = "PaymentChannelResult.Insurance")]
    Insurance {
        #[serde(default, rename = "insurerCode")]
        insurer_code: Option<String>,
        #[serde(default, rename = "insuranceNameI18n")]
        insurance_name_i18n: Option<I18nMap>,
    },
    #[serde(rename = "PaymentChannelResult.InsuranceV2")]
    InsuranceV2 {
        #[serde(default, rename = "insurerCode")]
        insurer_code: Option<String>,
        #[serde(default, rename = "insuranceNameI18n")]
        insurance_name_i18n: Option<I18nMap>,
    },
    // Coverage — insurance v3 has provider* instead of insurer*.
    #[serde(rename = "PaymentChannelResult.InsuranceV3")]
    InsuranceV3 {
        #[serde(default, rename = "providerName")]
        provider_name: Option<String>,
        #[serde(default, rename = "providerAbbreviation")]
        provider_abbreviation: Option<String>,
        #[serde(default, rename = "insuranceNameI18n")]
        insurance_name_i18n: Option<I18nMap>,
        #[serde(default, rename = "privilegeId")]
        privilege_id: Option<i64>,
    },

    // Coverage — employee benefit (not insurance).
    #[serde(rename = "PaymentChannelResult.EmployeeBenefit")]
    EmployeeBenefit {
        #[serde(default, rename = "companyName")]
        company_name: Option<String>,
    },
    #[serde(rename = "PaymentChannelResult.EmployeeBenefitV2")]
    EmployeeBenefitV2 {
        #[serde(default, rename = "companyName")]
        company_name: Option<String>,
    },

    // Coverage — campaign.
    #[serde(rename = "PaymentChannelResult.CampaignLocation")]
    CampaignLocation,

    /// Catch-all for variants we don't model. Mapper falls back to "Self pay".
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct I18nMap {
    #[serde(default)]
    pub en: Option<String>,
    // Other locales (th, etc.) intentionally unmodeled.
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
enum WireEnvelope {
    #[serde(rename = "Success")]
    Success { detail: PaymentDetail },
    #[serde(rename = "NotFound")]
    NotFound,
    #[serde(rename = "UnexpectedError")]
    UnexpectedError,
}

#[async_trait]
pub trait PaymentClientTrait: Send + Sync {
    async fn get_payment(&self, payment_tx_id: i64) -> AppResult<PaymentLookup>;
}

#[derive(Clone)]
pub struct PaymentClient {
    client: Client,
    base_uri: String,
}

impl PaymentClient {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build payment HTTP client"),
            base_uri,
        }
    }
}

#[async_trait]
impl PaymentClientTrait for PaymentClient {
    #[tracing::instrument(name = "payment.get_transaction_info", skip(self), fields(payment_tx_id = payment_tx_id))]
    async fn get_payment(&self, payment_tx_id: i64) -> AppResult<PaymentLookup> {
        let url = format!(
            "{}/payment/internal/transactions/{}",
            self.base_uri, payment_tx_id
        );
        debug!(%url, "calling payment upstream");

        let resp = match send_with_retry(&self.client, &url).await {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "payment upstream failed after retry");
                return Err(AppError::UpstreamError("payment".to_string()));
            }
        };

        let envelope: WireEnvelope = resp.json().await.map_err(|e| {
            warn!(error = %e, "payment upstream returned an unexpected response body");
            AppError::UpstreamError("payment".to_string())
        })?;

        Ok(match envelope {
            WireEnvelope::Success { detail } => PaymentLookup::Found(detail),
            WireEnvelope::NotFound => PaymentLookup::NotFound,
            WireEnvelope::UnexpectedError => {
                warn!("payment upstream returned UnexpectedError");
                return Err(AppError::UpstreamError("payment".to_string()));
            }
        })
    }
}

/// Send a GET request once, retry once on transient transport failure
/// (network error or HTTP 5xx). No retry on 4xx or successful response.
async fn send_with_retry(client: &Client, url: &str) -> Result<reqwest::Response, reqwest::Error> {
    match client.get(url).send().await {
        Ok(r) if r.status().is_server_error() => {
            warn!(status = %r.status(), %url, attempt = 1, "transport 5xx, retrying");
            client.get(url).send().await?.error_for_status()
        }
        Ok(r) if r.status().is_client_error() => {
            warn!(status = %r.status(), %url, "upstream returned client error, no retry");
            r.error_for_status()
        }
        Ok(r) => r.error_for_status(),
        Err(e) if e.is_connect() || e.is_timeout() || e.is_request() => {
            warn!(error = %e, %url, attempt = 1, "transport error, retrying");
            client.get(url).send().await?.error_for_status()
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn get_payment_uses_internal_transaction_path() {
        let upstream = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/payment/internal/transactions/1042"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "__type": "Success",
                "detail": {
                    "paymentTransactionId": 1042,
                    "paymentTransactionRefId": "PT-2026-001",
                    "amount": 1500,
                    "selectedChannelResult": null,
                    "couponProtocol": null
                }
            })))
            .mount(&upstream)
            .await;

        let client = PaymentClient::new(upstream.uri());
        let result = client.get_payment(1042).await.unwrap();

        match result {
            PaymentLookup::Found(detail) => {
                assert_eq!(detail.payment_transaction_id, 1042);
                assert_eq!(detail.payment_transaction_ref_id, "PT-2026-001");
            }
            PaymentLookup::NotFound => panic!("expected payment detail"),
        }
    }
}
