use crate::core::error::{AppError, AppResult};
use crate::core::kms::KmsClient;
use crate::model::onboarding::{Address, Documents, Education, SelectedWorkPlace, Specialty};
use crate::model::ref_data::{
    AcademicPosition, District, MedicalSchool, PostalCode, Profession, Province, SubDistrict,
    WorkPlace,
};
use crate::module::profile::handler::DoctorProfile;
use async_trait::async_trait;
use sqlx::{FromRow, PgPool};
use std::sync::Arc;
use tdh_protocol::common::Localized;
use tracing::warn;

#[async_trait]
pub trait ProfileRepoTrait: Send + Sync {
    async fn get_doctor_profile(&self, account_id: i32) -> AppResult<Option<DoctorProfile>>;
    async fn get_doctor_uuid(&self, account_id: i32) -> AppResult<Option<String>>;
}

#[derive(Clone)]
pub struct ProfileRepoImp {
    pool: PgPool,
    kms: Arc<dyn KmsClient>,
}

impl ProfileRepoImp {
    pub fn new(pool: PgPool, kms: Arc<dyn KmsClient>) -> Self {
        Self { pool, kms }
    }
}

#[derive(FromRow)]
struct DoctorProfileRow {
    citizen_id: String,
    profession: serde_json::Value,
    academic_position: serde_json::Value,
    first_name: serde_json::Value,
    last_name: serde_json::Value,
    license_number: String,
    medical_school: serde_json::Value,
    specialty: serde_json::Value,
    additional_specialties: serde_json::Value,
    department_id: i32,
    special_interests: Vec<String>,
    address_detail: String,
    sub_district: serde_json::Value,
    district: serde_json::Value,
    province: serde_json::Value,
    postal_code: i32,
    primary_workplace: serde_json::Value,
    additional_workplace: serde_json::Value,
    profile_image_url: String,
    id_card_image_url: String,
    book_bank_image_url: String,
    medical_license_image_url: String,
    education_license_image_url: Vec<String>,
}
/// Deserialize a JSONB column into `T`, logging and falling back on decode errors
/// so a single malformed field never fails the whole row read.
fn decode<T: serde::de::DeserializeOwned>(
    value: serde_json::Value,
    field: &str,
    fallback: impl FnOnce() -> T,
) -> T {
    serde_json::from_value(value).unwrap_or_else(|e| {
        warn!(error = %e, field, "failed to deserialize doctor profile field");
        fallback()
    })
}

impl DoctorProfileRow {
    fn into_profile(self, citizen_id: String) -> DoctorProfile {
        let row = self;

        let empty_loc = || Localized {
            th: String::new(),
            en: String::new(),
        };

        let first_name = decode(row.first_name, "first_name", empty_loc);
        let last_name = decode(row.last_name, "last_name", empty_loc);
        let profession = decode(row.profession, "profession", Profession::default);
        let academic_position = decode(
            row.academic_position,
            "academic_position",
            AcademicPosition::default,
        );
        let medical_school = decode(row.medical_school, "medical_school", MedicalSchool::default);
        let sub_district = decode(row.sub_district, "sub_district", SubDistrict::default);
        let district = decode(row.district, "district", District::default);
        let province = decode(row.province, "province", Province::default);
        let primary_workplace: Vec<WorkPlace> =
            decode(row.primary_workplace, "primary_workplace", Vec::new);
        let additional_workplace: Vec<WorkPlace> =
            decode(row.additional_workplace, "additional_workplace", Vec::new);
        let specialties = decode(row.specialty, "specialty", Specialty::default);
        let additional_specialties: Vec<Specialty> = decode(
            row.additional_specialties,
            "additional_specialties",
            Vec::new,
        );
        let special_interests = row.special_interests;
        let postal_code = PostalCode {
            id: row.postal_code,
            district_id: district.id,
        };

        DoctorProfile {
            citizen_id,
            profession,
            academic_position,
            first_name,
            last_name,
            department: row.department_id,
            address: Address {
                address_detail: row.address_detail,
                sub_district,
                district,
                province,
                postal_code,
            },
            work_place: SelectedWorkPlace {
                primary: primary_workplace,
                additional: additional_workplace,
            },
            education: Education {
                license_number: row.license_number,
                medical_school,
                specialties,
                additional_specialties,
                special_interests,
            },
            documents: Documents {
                profile_image_url: row.profile_image_url,
                id_card_image_url: row.id_card_image_url,
                book_bank_image_url: row.book_bank_image_url,
                med_license_image_url: row.medical_license_image_url,
                certificate_image_urls: row.education_license_image_url,
            },
        }
    }
}
#[async_trait]
impl ProfileRepoTrait for ProfileRepoImp {
    async fn get_doctor_profile(&self, doctor_id: i32) -> AppResult<Option<DoctorProfile>> {
        let row = sqlx::query_as::<_, DoctorProfileRow>(
            r#"
            SELECT * FROM get_doctor_profile($1)
            "#,
        )
        .bind(doctor_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {}", e)))?;

        match row {
            Some(row) => {
                // citizen_id is KMS-encrypted at rest; decrypt on demand for display.
                let citizen_id = if row.citizen_id.is_empty() {
                    String::new()
                } else {
                    self.kms.decrypt(&row.citizen_id).await?
                };
                Ok(Some(row.into_profile(citizen_id)))
            }
            None => Ok(None),
        }
    }
    async fn get_doctor_uuid(&self, account_id: i32) -> AppResult<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT doctor_id::text FROM doctor_profile WHERE doctor_account_id = $1",
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {}", e)))?;
        Ok(row.map(|(id,)| id))
    }
}
