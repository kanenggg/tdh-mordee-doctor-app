use crate::core::error::{AppError, AppResult};
use crate::module::profile::configuration::models::{
    ChannelType, DoctorConfiguration, Fee, LanguageCode,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorConsultationConfig {
    pub supported_languages: Option<Vec<LanguageCode>>,
    pub channel_types: Option<Vec<ChannelType>>,
    pub duration_minutes: Option<i32>,
    pub doctor_fee_amount: Option<f64>,
}

pub struct ConsultationConfigInfo {
    pub supported_languages: Option<Vec<LanguageCode>>,
    pub channel_types: Option<Vec<ChannelType>>,
    pub duration_minutes: Option<i32>,
    pub doctor_fee_amount: Option<f64>,
}

impl From<DoctorConsultationConfig> for ConsultationConfigInfo {
    fn from(config: DoctorConsultationConfig) -> Self {
        Self {
            supported_languages: config.supported_languages,
            channel_types: config.channel_types,
            duration_minutes: config.duration_minutes,
            doctor_fee_amount: config.doctor_fee_amount,
        }
    }
}

pub fn build_doctor_configuration(
    request: ConsultationConfigInfo,
) -> AppResult<DoctorConfiguration> {
    let channel = required_values(request.channel_types, "channel")?;
    let language = required_values(request.supported_languages, "language")?;

    if let Some(duration) = request.duration_minutes {
        if !matches!(duration, 15 | 25 | 50) {
            return Err(AppError::BadRequest(
                "duration must be one of 15, 25, or 50".to_string(),
            ));
        }
    }
    if let Some(amount) = request.doctor_fee_amount {
        if !amount.is_finite() || amount < 0.0 {
            return Err(AppError::BadRequest(
                "doctor fee amount must be greater than or equal to 0".to_string(),
            ));
        }
    }

    Ok(DoctorConfiguration {
        channel,
        duration: request.duration_minutes,
        fee: Fee {
            amount: request.doctor_fee_amount,
            currency: "THB".to_string(),
        },
        language,
    })
}

fn required_values<T>(values: Option<Vec<T>>, field: &str) -> AppResult<Vec<T>> {
    match values {
        Some(values) if !values.is_empty() => Ok(values),
        _ => Err(AppError::BadRequest(format!(
            "{field} must contain at least one value"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> ConsultationConfigInfo {
        ConsultationConfigInfo {
            supported_languages: Some(vec![LanguageCode::Th, LanguageCode::En]),
            channel_types: Some(vec![ChannelType::Voice, ChannelType::Chat]),
            duration_minutes: Some(25),
            doctor_fee_amount: Some(200.0),
        }
    }

    #[test]
    fn builds_valid_doctor_configuration() {
        let config = build_doctor_configuration(valid_config()).unwrap();

        assert_eq!(config.language, vec![LanguageCode::Th, LanguageCode::En]);
        assert_eq!(config.channel, vec![ChannelType::Voice, ChannelType::Chat]);
        assert_eq!(config.duration, Some(25));
        assert_eq!(config.fee.amount, Some(200.0));
        assert_eq!(config.fee.currency, "THB");
    }

    #[test]
    fn rejects_empty_languages() {
        let mut request = valid_config();
        request.supported_languages = Some(vec![]);

        assert!(matches!(
            build_doctor_configuration(request),
            Err(AppError::BadRequest(message)) if message == "language must contain at least one value"
        ));
    }

    #[test]
    fn rejects_empty_channels() {
        let mut request = valid_config();
        request.channel_types = Some(vec![]);

        assert!(matches!(
            build_doctor_configuration(request),
            Err(AppError::BadRequest(message)) if message == "channel must contain at least one value"
        ));
    }

    #[test]
    fn rejects_invalid_duration() {
        let mut request = valid_config();
        request.duration_minutes = Some(30);

        assert!(matches!(
            build_doctor_configuration(request),
            Err(AppError::BadRequest(message)) if message == "duration must be one of 15, 25, or 50"
        ));
    }

    #[test]
    fn rejects_negative_fee() {
        let mut request = valid_config();
        request.doctor_fee_amount = Some(-1.0);

        assert!(matches!(
            build_doctor_configuration(request),
            Err(AppError::BadRequest(message)) if message == "doctor fee amount must be greater than or equal to 0"
        ));
    }

    #[test]
    fn rejects_non_finite_fee() {
        let mut request = valid_config();
        request.doctor_fee_amount = Some(f64::NAN);

        assert!(matches!(
            build_doctor_configuration(request),
            Err(AppError::BadRequest(message)) if message == "doctor fee amount must be greater than or equal to 0"
        ));
    }
}
