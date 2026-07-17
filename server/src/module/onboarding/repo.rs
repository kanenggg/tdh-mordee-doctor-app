use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Type};
use std::sync::Arc;
use tdh_protocol::common::Localized;
use tracing::warn;

use crate::core::error::{AppError, AppResult};
use crate::core::kms::KmsClient;
use crate::model::onboarding::{
    Address, Documents, Education, OnBoardingRequest, OnBoardingStatus, OnBoardingStub,
    SelectedWorkPlace, Specialty,
};
use crate::model::ref_data::{
    AcademicPosition, District, MedicalSchool, PostalCode, Profession, Province, SubDistrict,
    WorkPlace,
};
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "PascalCase")]
#[sqlx(type_name = "doctor_profile_status_enum", rename_all = "PascalCase")]
pub enum OnBoardingStatusDb {
    Draft,
    PendingApproval,
    Approved,
    Rejected,
    Deactivated,
}

impl OnBoardingStatusDb {
    pub fn to_domain(self, reason: Option<String>) -> OnBoardingStatus {
        match self {
            Self::Draft => OnBoardingStatus::Draft,
            Self::PendingApproval => OnBoardingStatus::PendingApproval,
            Self::Approved => OnBoardingStatus::Approved,
            Self::Rejected => OnBoardingStatus::Rejected {
                reason: reason.unwrap_or_default(),
            },
            Self::Deactivated => OnBoardingStatus::Deactivated {
                reason: reason.unwrap_or_default(),
            },
        }
    }
}

#[async_trait]
pub trait OnBoardingRepo: Send + Sync {
    async fn get_doctor_profile_draft(&self, doctor_id: i32) -> AppResult<Option<OnBoardingStub>>;
    async fn save_doctor_profile_draft(
        &self,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        request: &OnBoardingRequest,
    ) -> AppResult<()>;
    async fn submit_doctor_profile_draft(
        &self,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        request: &OnBoardingRequest,
    ) -> AppResult<()>;
    async fn get_onboarding_status(
        &self,
        doctor_account_id: i32,
    ) -> AppResult<Option<OnBoardingStatus>>;
}

#[derive(Clone)]
pub struct OnBoardingRepoImp {
    pool: PgPool,
    kms: Arc<dyn KmsClient>,
}

impl OnBoardingRepoImp {
    pub fn new(pool: PgPool, kms: Arc<dyn KmsClient>) -> Self {
        Self { pool, kms }
    }

    /// Empty citizen_id (partial draft) is stored as NULL; otherwise KMS-encrypt it.
    async fn encrypt_citizen_id(&self, citizen_id: &str) -> AppResult<Option<String>> {
        if citizen_id.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.kms.encrypt(citizen_id).await?))
        }
    }
}

#[derive(FromRow)]
struct OnBoardingRow {
    citizen_id: Option<String>,
    profession: serde_json::Value,
    academic_position: serde_json::Value,
    first_name: serde_json::Value,
    last_name: serde_json::Value,
    license_number: String,
    medical_school: serde_json::Value,
    specialty: serde_json::Value,
    additional_specialties: serde_json::Value,
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
    status: OnBoardingStatusDb,
}

/// Deserialize a JSONB column into `T`, logging and falling back on decode errors
/// so a single malformed field never fails the whole row read.
fn decode<T: serde::de::DeserializeOwned>(
    value: serde_json::Value,
    field: &str,
    fallback: impl FnOnce() -> T,
) -> T {
    serde_json::from_value(value).unwrap_or_else(|e| {
        warn!(error = %e, field, "failed to deserialize onboarding field");
        fallback()
    })
}

impl From<OnBoardingRow> for OnBoardingStub {
    fn from(row: OnBoardingRow) -> Self {
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

        Self {
            // citizen_id ciphertext is KMS-decrypted by the repo method (KMS is async,
            // not available in this stateless From impl).
            citizen_id: String::new(),
            profession,
            academic_position,
            first_name,
            last_name,
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
            status: row.status.to_domain(None),
        }
    }
}

#[async_trait]
impl OnBoardingRepo for OnBoardingRepoImp {
    async fn get_doctor_profile_draft(&self, doctor_id: i32) -> AppResult<Option<OnBoardingStub>> {
        let row = sqlx::query_as::<_, OnBoardingRow>(
            r#"
            SELECT * FROM get_doctor_profile_draft($1)
            "#,
        )
        .bind(doctor_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {}", e)))?;

        match row {
            Some(row) => {
                let citizen_id = match &row.citizen_id {
                    Some(ciphertext) => self.kms.decrypt(ciphertext).await?,
                    None => String::new(),
                };
                let mut stub = OnBoardingStub::from(row);
                stub.citizen_id = citizen_id;
                Ok(Some(stub))
            }
            None => Ok(None),
        }
    }

    async fn get_onboarding_status(
        &self,
        doctor_account_id: i32,
    ) -> AppResult<Option<OnBoardingStatus>> {
        let row: Option<(OnBoardingStatusDb, Option<String>)> =
            sqlx::query_as("SELECT status, status_reason FROM get_onboarding_status($1)")
                .bind(doctor_account_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| AppError::InternalError(format!("Database error: {}", e)))?;

        Ok(row.map(|(status, reason)| status.to_domain(reason)))
    }

    async fn save_doctor_profile_draft(
        &self,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        request: &OnBoardingRequest,
    ) -> AppResult<()> {
        use anyhow::Context;

        let profession_json =
            serde_json::to_value(&request.profession).context("serialize profession")?;
        let academic_pos_json = serde_json::to_value(&request.academic_position)
            .context("serialize academic_position")?;
        let first_name_json =
            serde_json::to_value(&request.first_name).context("serialize first_name")?;
        let last_name_json =
            serde_json::to_value(&request.last_name).context("serialize last_name")?;
        let medical_school_json = serde_json::to_value(&request.education.medical_school)
            .context("serialize medical_school")?;
        let sub_district_json = serde_json::to_value(&request.address.sub_district)
            .context("serialize sub_district")?;
        let district_json =
            serde_json::to_value(&request.address.district).context("serialize district")?;
        let province_json =
            serde_json::to_value(&request.address.province).context("serialize province")?;
        let primary_wp_json = serde_json::to_value(&request.work_place.primary)
            .context("serialize primary_workplace")?;
        let additional_wp_json = serde_json::to_value(&request.work_place.additional)
            .context("serialize additional_workplace")?;
        let specialty_json =
            serde_json::to_value(&request.education.specialties).context("serialize specialty")?;
        let additional_specialties_json =
            serde_json::to_value(&request.education.additional_specialties)
                .context("serialize additional_specialties")?;

        let citizen_id = self.encrypt_citizen_id(&request.citizen_id).await?;

        sqlx::query(
            r#"
            SELECT save_doctor_profile_draft(
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                $16, $17, $18, $19, $20, $21, $22, $23, $24
            )
            "#,
        )
        .bind(doctor_account_id) // $1  p_doctor_account_id
        .bind(doctor_profile_id) // $2  p_doctor_profile_id
        .bind(&citizen_id) // $3  p_citizen_id
        .bind(profession_json) // $4  p_profession
        .bind(academic_pos_json) // $5  p_academic_position
        .bind(first_name_json) // $6  p_first_name
        .bind(last_name_json) // $7  p_last_name
        .bind(&request.education.license_number) // $8  p_license_number
        .bind(medical_school_json) // $9  p_medical_school
        .bind(specialty_json) // $10 p_specialty (merged)
        .bind(&request.education.special_interests) // $11 p_special_interests
        .bind(&request.address.address_detail) // $12 p_address_detail
        .bind(sub_district_json) // $13 p_sub_district
        .bind(district_json) // $14 p_district
        .bind(province_json) // $15 p_province
        .bind(request.address.postal_code.id) // $16 p_postal_code
        .bind(primary_wp_json) // $17 p_primary_workplace
        .bind(additional_wp_json) // $18 p_additional_workplace
        .bind(&request.documents.profile_image_url) // $19 p_profile_image_url
        .bind(&request.documents.id_card_image_url) // $20 p_id_card_image_url
        .bind(&request.documents.book_bank_image_url) // $21 p_book_bank_image_url
        .bind(&request.documents.med_license_image_url) // $22 p_medical_license_image_url
        .bind(&request.documents.certificate_image_urls) // $23 p_education_certificate_image_urls
        .bind(additional_specialties_json) // $24 p_additional_specialties
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn submit_doctor_profile_draft(
        &self,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        request: &OnBoardingRequest,
    ) -> AppResult<()> {
        use anyhow::Context;

        let profession_json =
            serde_json::to_value(&request.profession).context("serialize profession")?;
        let academic_pos_json = serde_json::to_value(&request.academic_position)
            .context("serialize academic_position")?;
        let first_name_json =
            serde_json::to_value(&request.first_name).context("serialize first_name")?;
        let last_name_json =
            serde_json::to_value(&request.last_name).context("serialize last_name")?;
        let medical_school_json = serde_json::to_value(&request.education.medical_school)
            .context("serialize medical_school")?;
        let sub_district_json = serde_json::to_value(&request.address.sub_district)
            .context("serialize sub_district")?;
        let district_json =
            serde_json::to_value(&request.address.district).context("serialize district")?;
        let province_json =
            serde_json::to_value(&request.address.province).context("serialize province")?;
        let primary_wp_json = serde_json::to_value(&request.work_place.primary)
            .context("serialize primary_workplace")?;
        let additional_wp_json = serde_json::to_value(&request.work_place.additional)
            .context("serialize additional_workplace")?;
        let specialty_json =
            serde_json::to_value(&request.education.specialties).context("serialize specialty")?;
        let additional_specialties_json =
            serde_json::to_value(&request.education.additional_specialties)
                .context("serialize additional_specialties")?;

        let citizen_id = self.encrypt_citizen_id(&request.citizen_id).await?;

        sqlx::query(
            r#"
            SELECT submit_doctor_profile_draft(
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                $16, $17, $18, $19, $20, $21, $22, $23, $24
            )
            "#,
        )
        .bind(doctor_account_id) // $1  p_doctor_account_id
        .bind(doctor_profile_id) // $2  p_doctor_profile_id
        .bind(&citizen_id) // $3  p_citizen_id
        .bind(profession_json) // $4  p_profession
        .bind(academic_pos_json) // $5  p_academic_position
        .bind(first_name_json) // $6  p_first_name
        .bind(last_name_json) // $7  p_last_name
        .bind(&request.education.license_number) // $8  p_license_number
        .bind(medical_school_json) // $9  p_medical_school
        .bind(specialty_json) // $10 p_specialty (merged)
        .bind(&request.education.special_interests) // $11 p_special_interests
        .bind(&request.address.address_detail) // $12 p_address_detail
        .bind(sub_district_json) // $13 p_sub_district
        .bind(district_json) // $14 p_district
        .bind(province_json) // $15 p_province
        .bind(request.address.postal_code.id) // $16 p_postal_code
        .bind(primary_wp_json) // $17 p_primary_workplace
        .bind(additional_wp_json) // $18 p_additional_workplace
        .bind(&request.documents.profile_image_url) // $19 p_profile_image_url
        .bind(&request.documents.id_card_image_url) // $20 p_id_card_image_url
        .bind(&request.documents.book_bank_image_url) // $21 p_book_bank_image_url
        .bind(&request.documents.med_license_image_url) // $22 p_medical_license_image_url
        .bind(&request.documents.certificate_image_urls) // $23 p_education_certificate_image_urls
        .bind(additional_specialties_json) // $24 p_additional_specialties
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
