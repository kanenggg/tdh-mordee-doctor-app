use super::language::Language;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ─── Page Token ──────────────────────────────────────────────────────────────

/// Cursor for keyset pagination. Encodes the last-seen (score, uuid) pair.
#[derive(Debug, Serialize, Deserialize)]
pub struct PageToken {
    /// Last doctor's ranking score
    pub s: i32,
    /// Last doctor's UUID
    pub u: Uuid,
}

impl PageToken {
    pub fn new(score: i32, uuid: Uuid) -> Self {
        Self { s: score, u: uuid }
    }

    pub fn encode(&self) -> String {
        let json = serde_json::to_string(self).expect("PageToken serialization cannot fail");
        URL_SAFE_NO_PAD.encode(json.as_bytes())
    }

    pub fn decode(token: &str) -> Result<Self, PageTokenError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(token)
            .map_err(|_| PageTokenError::InvalidBase64)?;
        let json = String::from_utf8(bytes).map_err(|_| PageTokenError::InvalidUtf8)?;
        serde_json::from_str(&json).map_err(|_| PageTokenError::InvalidJson)
    }
}

#[derive(Debug)]
pub enum PageTokenError {
    InvalidBase64,
    InvalidUtf8,
    InvalidJson,
}

impl std::fmt::Display for PageTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBase64 => write!(f, "Invalid page token encoding"),
            Self::InvalidUtf8 => write!(f, "Invalid page token content"),
            Self::InvalidJson => write!(f, "Malformed page token"),
        }
    }
}

// ─── Query Parameters ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, IntoParams)]
pub struct RankingQuery {
    #[serde(default = "default_size")]
    pub size: u32,
    #[serde(rename = "pageToken")]
    pub page_token: Option<String>,
}

fn default_size() -> u32 {
    30
}

// ─── Database Row ────────────────────────────────────────────────────────────

/// Flat row returned from the ranking SQL query (joined from multiple tables).
#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct DoctorRow {
    pub doctor_id: Uuid,
    pub firstname: serde_json::Value, // JSONB: {"en": "...", "th": "..."}
    pub lastname: serde_json::Value,  // JSONB: {"en": "...", "th": "..."}
    pub profile_image_url: String,
    pub supported_languages: Vec<String>, // Cast from enum array
    pub department_id: Option<i32>,
    pub department_name: Option<serde_json::Value>,
    pub department_counseling_areas: Option<serde_json::Value>,
    pub score: i32,
    pub rating: Option<f64>,
    pub case_amount: Option<i32>,
    pub fee_amount: Option<f64>,
    pub default_duration: Option<i32>,
    pub channel_types: Vec<String>,
    pub specialty: serde_json::Value,
    pub work_place: serde_json::Value,
    pub additional_workplace: serde_json::Value,
    // ranked / iRanked are computed at runtime from Redis sorted set positions
}

/// Specialty info from doctor_specialty join.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpecialtyInfo {
    pub id: i32,
    pub name: String,
    pub lang_code: String,
}

/// Channel info composed from doctor_consultation_config channel, fee, and duration.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct ChannelInfo {
    #[serde(rename = "type")]
    pub channel_type: String,
    pub duration: i32,
    pub price: f64,
    pub currency: String,
}

/// Workplace info.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkplaceInfo {
    pub id: i32,
    pub name: String,
    pub lang_code: String,
}

// ─── API Response Types ──────────────────────────────────────────────────────

/// Single doctor profile in the ranking list.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorProfile {
    pub usid: String,
    pub name: Vec<LocalizedName>,
    pub profile_image: String,
    pub specialties: Vec<SpecialtyInfo>,
    pub channels: Vec<ChannelInfo>,
    pub work_place: Vec<WorkplaceInfo>,
    #[serde(default)]
    pub counseling_areas: Vec<LocalizedName>,
    #[serde(default)]
    pub work_experience: Vec<LocalizedName>,
    pub specialty_desc: Vec<LocalizedName>,
    pub consultation_case: i32,
    pub rating: f64,
    pub available_language: Vec<String>,
    pub consultation_fee: f64,
    pub consultation_duration: i32,
    #[serde(skip)]
    #[schema(ignore)]
    pub department_id: Option<i32>,
    pub associate_privileges: Vec<AssociatePrivilege>,
    pub score: i32,
    pub ranked: i32, // Current position in scheduled sorted set (from Redis)
    #[serde(rename = "iRanked")]
    pub i_ranked: i32, // Current position in instant sorted set (from Redis)
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedName {
    pub lang_code: String,
    pub name: String,
}

/// Paging metadata in list responses.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PagingMetaData {
    pub size: u32,
    pub total: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// List response data.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorListData {
    pub doctors: Vec<DoctorProfile>,
    pub paging_meta_data: PagingMetaData,
}

/// Top-level list response envelope (matches legacy contract).
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorListResponse {
    pub message: String,
    pub return_type: String,
    pub data: DoctorListData,
}

/// Top-level single-doctor response envelope.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorProfileResponse {
    pub message: String,
    pub return_type: String,
    pub data: DoctorProfile,
}

// ─── Privilege types (internal — deserialized from privilege-man) ────────────

/// Raw privilege benefit from privilege-man `/internal/v1/benefit/list`.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PrivilegeBenefit {
    pub privilege_id: i64,
    pub privilege_display_name: String,
    pub provider_id: i64,
    pub provider_name: String,
    pub provider_abbreviation: String,
    pub package_type_id: i32,
    pub package_type_name: Option<String>,
    pub benefits: Vec<String>,
    pub instruction_html: String,
    pub company_logo_url: Option<String>,
}

/// Response envelope from privilege-man internal API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivilegeBenefitApiResponse {
    pub privilege_benefits: Vec<PrivilegeBenefit>,
}

// ─── Privilege types (output — matches legacy API contract) ─────────────────

/// Insurance privilege attached to a doctor profile (legacy contract).
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssociatePrivilege {
    pub id: i64,
    pub name: String,
    pub logo_url: String,
    pub is_default: bool,
    pub is_connect: bool,
    pub discount_percent: i32,
    pub benefit_description: String,
    pub privilege_description: String,
    pub policy_types: Vec<PolicyType>,
}

/// Policy type nested inside an associate privilege (legacy contract).
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyType {
    pub policy_type_id: i32,
    pub insurance_type: String,
    pub display_name: String,
    pub instruction_text: String,
    pub consent_url: String,
    pub legacy_package_id: i32,
}

impl From<PrivilegeBenefit> for AssociatePrivilege {
    fn from(pb: PrivilegeBenefit) -> Self {
        let benefit_description = pb.benefits.join(", ");
        let insurance_type = pb.package_type_name.unwrap_or_default();
        let display_name = pb.privilege_display_name;
        let instruction = pb.instruction_html;

        Self {
            id: pb.privilege_id,
            name: display_name.clone(),
            logo_url: pb.company_logo_url.unwrap_or_default(),
            is_default: false,
            is_connect: false,
            discount_percent: 0,
            benefit_description,
            privilege_description: instruction.clone(),
            policy_types: vec![PolicyType {
                policy_type_id: pb.package_type_id,
                insurance_type,
                display_name,
                instruction_text: instruction,
                consent_url: String::new(),
                legacy_package_id: 0,
            }],
        }
    }
}

// ─── Warm-up types ───────────────────────────────────────────────────────────

/// Minimal doctor info for Redis warm-up.
#[derive(Debug, sqlx::FromRow)]
pub struct RankedDoctorInfo {
    pub doctor_id: Uuid,
    pub score: i32,
    pub instant_mode_enabled: bool,
    pub schedule_mode_enabled: bool,
}

// ─── Helper functions ────────────────────────────────────────────────────────

/// Map internal language code to legacy format.
pub fn map_language_code(code: &str) -> String {
    match code {
        "th" => "th-TH".to_string(),
        "en" => "en-US".to_string(),
        other => other.to_string(),
    }
}

/// Build localized name entry from JSONB name fields for the requested language.
/// Falls back to Thai if the requested language key is not present.
pub fn build_doctor_name(
    firstname: &serde_json::Value,
    lastname: &serde_json::Value,
    lang: &Language,
) -> Vec<LocalizedName> {
    let (first_map, last_map) = match (firstname.as_object(), lastname.as_object()) {
        (Some(f), Some(l)) => (f, l),
        _ => return vec![],
    };

    let key = lang.json_key();
    let fallback = Language::Thai.json_key();

    let effective_key = if first_map.contains_key(key) && last_map.contains_key(key) {
        key
    } else if first_map.contains_key(fallback) && last_map.contains_key(fallback) {
        fallback
    } else {
        return vec![];
    };

    let name = match (
        first_map.get(effective_key).and_then(|v| v.as_str()),
        last_map.get(effective_key).and_then(|v| v.as_str()),
    ) {
        (Some(f), Some(l)) => format!("{} {}", f, l),
        _ => return vec![],
    };

    vec![LocalizedName {
        lang_code: map_language_code(effective_key),
        name,
    }]
}

/// Build localized specialty descriptions from `doctor_profile.specialty`.
pub fn build_specialty_desc(specialty: &serde_json::Value, lang: &Language) -> Vec<LocalizedName> {
    specialty_values(specialty)
        .into_iter()
        .filter_map(|item| {
            specialty_text(item, lang).map(|name| LocalizedName {
                lang_code: lang.lang_code().to_string(),
                name,
            })
        })
        .collect()
}

fn specialty_values(specialty: &serde_json::Value) -> Vec<&serde_json::Value> {
    if let Some(items) = specialty.as_array() {
        return items.iter().collect();
    }

    if specialty.as_object().is_some() {
        return vec![specialty];
    }

    vec![]
}

fn specialty_text(item: &serde_json::Value, lang: &Language) -> Option<String> {
    localized_text(item.get("name"), lang).or_else(|| localized_text(item.get("description"), lang))
}

fn localized_text(value: Option<&serde_json::Value>, lang: &Language) -> Option<String> {
    let value = value?;
    if let Some(text) = value.as_str() {
        return non_empty(text);
    }

    value
        .get(lang.json_key())
        .or_else(|| value.get("th"))
        .or_else(|| value.get("en"))
        .and_then(|v| v.as_str())
        .and_then(non_empty)
}

fn non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_token_roundtrip() {
        let uuid = Uuid::parse_str("82e6b8ce-071c-4752-83b5-5cd552ef7ffb").unwrap();
        let token = PageToken::new(85, uuid);
        let encoded = token.encode();
        let decoded = PageToken::decode(&encoded).unwrap();
        assert_eq!(decoded.s, 85);
        assert_eq!(decoded.u, uuid);
    }

    #[test]
    fn test_page_token_invalid() {
        assert!(PageToken::decode("not-valid-base64!!!").is_err());
        assert!(PageToken::decode(&URL_SAFE_NO_PAD.encode(b"not json")).is_err());
    }

    #[test]
    fn test_map_language_code() {
        assert_eq!(map_language_code("th"), "th-TH");
        assert_eq!(map_language_code("en"), "en-US");
        assert_eq!(map_language_code("ja"), "ja");
    }

    #[test]
    fn test_privilege_benefit_to_associate_privilege() {
        let pb = PrivilegeBenefit {
            privilege_id: 1001,
            privilege_display_name: "AIA Individual".to_string(),
            provider_id: 101,
            provider_name: "AIA".to_string(),
            provider_abbreviation: "AIA".to_string(),
            package_type_id: 1,
            package_type_name: Some("Individual".to_string()),
            benefits: vec!["Claimable".to_string(), "15% Off".to_string()],
            instruction_html: "<p>Terms</p>".to_string(),
            company_logo_url: Some("https://example.com/logo.png".to_string()),
        };

        let ap: AssociatePrivilege = pb.into();

        assert_eq!(ap.id, 1001);
        assert_eq!(ap.name, "AIA Individual");
        assert_eq!(ap.logo_url, "https://example.com/logo.png");
        assert!(!ap.is_default);
        assert!(!ap.is_connect);
        assert_eq!(ap.discount_percent, 0);
        assert_eq!(ap.benefit_description, "Claimable, 15% Off");
        assert_eq!(ap.privilege_description, "<p>Terms</p>");
        assert_eq!(ap.policy_types.len(), 1);
        assert_eq!(ap.policy_types[0].policy_type_id, 1);
        assert_eq!(ap.policy_types[0].insurance_type, "Individual");
        assert_eq!(ap.policy_types[0].display_name, "AIA Individual");
        assert_eq!(ap.policy_types[0].instruction_text, "<p>Terms</p>");
        assert_eq!(ap.policy_types[0].consent_url, "");
        assert_eq!(ap.policy_types[0].legacy_package_id, 0);
    }

    #[test]
    fn test_privilege_benefit_to_associate_privilege_with_none_fields() {
        let pb = PrivilegeBenefit {
            privilege_id: 2005,
            privilege_display_name: "CPF".to_string(),
            provider_id: 205,
            provider_name: "CPF".to_string(),
            provider_abbreviation: "CPF".to_string(),
            package_type_id: 3,
            package_type_name: None,
            benefits: vec![],
            instruction_html: String::new(),
            company_logo_url: None,
        };

        let ap: AssociatePrivilege = pb.into();

        assert_eq!(ap.logo_url, "");
        assert_eq!(ap.benefit_description, "");
        assert_eq!(ap.privilege_description, "");
        assert_eq!(ap.policy_types[0].insurance_type, "");
    }

    #[test]
    fn test_build_doctor_name_english() {
        let firstname = serde_json::json!({"en": "John", "th": "จอห์น"});
        let lastname = serde_json::json!({"en": "Doe", "th": "โด"});
        let result = build_doctor_name(&firstname, &lastname, &Language::English);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].lang_code, "en-US");
        assert_eq!(result[0].name, "John Doe");
    }

    #[test]
    fn test_build_doctor_name_thai() {
        let firstname = serde_json::json!({"en": "John", "th": "จอห์น"});
        let lastname = serde_json::json!({"en": "Doe", "th": "โด"});
        let result = build_doctor_name(&firstname, &lastname, &Language::Thai);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].lang_code, "th-TH");
        assert_eq!(result[0].name, "จอห์น โด");
    }

    #[test]
    fn test_build_doctor_name_fallback() {
        let firstname = serde_json::json!({"th": "จอห์น"});
        let lastname = serde_json::json!({"th": "โด"});
        let result = build_doctor_name(&firstname, &lastname, &Language::English);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].lang_code, "th-TH");
        assert_eq!(result[0].name, "จอห์น โด");
    }

    #[test]
    fn test_build_doctor_name_empty_when_no_data() {
        let firstname = serde_json::json!({});
        let lastname = serde_json::json!({});
        let result = build_doctor_name(&firstname, &lastname, &Language::English);
        assert!(result.is_empty());
    }

    #[test]
    fn test_build_specialty_desc_from_specialty_object() {
        let specialty = serde_json::json!({
            "id": 208,
            "name": {"th": "สุขภาพจิต", "en": "Mental Health"}
        });

        let result_en = build_specialty_desc(&specialty, &Language::English);
        assert_eq!(result_en.len(), 1);
        assert_eq!(result_en[0].lang_code, "en-US");
        assert_eq!(result_en[0].name, "Mental Health");

        let result_th = build_specialty_desc(&specialty, &Language::Thai);
        assert_eq!(result_th.len(), 1);
        assert_eq!(result_th[0].lang_code, "th-TH");
        assert_eq!(result_th[0].name, "สุขภาพจิต");
    }

    #[test]
    fn test_build_specialty_desc_from_specialty_array() {
        let specialty = serde_json::json!([
            {"id": 208, "name": {"th": "สุขภาพจิต", "en": "Mental Health"}}
        ]);

        let result = build_specialty_desc(&specialty, &Language::English);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].lang_code, "en-US");
        assert_eq!(result[0].name, "Mental Health");
    }

    #[test]
    fn test_build_specialty_desc_empty_for_missing_name() {
        let result = build_specialty_desc(&serde_json::json!({}), &Language::English);
        assert!(result.is_empty());
    }
}
