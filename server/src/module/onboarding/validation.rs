use crate::{
    core::error::{AppError, AppResult},
    model::onboarding::{OnBoardingStatus, OnBoardingStub},
};

#[derive(Clone)]
pub struct OnboardingValidatorImp;

impl OnboardingValidatorImp {
    pub fn new() -> Self {
        Self
    }

    fn validate_text_fields(info: &OnBoardingStub) -> AppResult<()> {
        validate_thai_citizen_id(&info.citizen_id)?;
        require_non_empty(
            &info.address.address_detail,
            "address.address_detail",
            "Address detail",
        )?;
        require_non_empty(
            &info.education.license_number,
            "education.license_number",
            "License number",
        )?;
        Ok(())
    }

    fn validate_required_ids(info: &OnBoardingStub) -> AppResult<()> {
        require_positive(info.profession.id, "profession", "Profession")?;
        require_positive(
            info.academic_position.id,
            "academic_position",
            "Academic position",
        )?;
        require_positive(
            info.address.sub_district.id,
            "address.sub_district",
            "Sub-district",
        )?;
        require_positive(info.address.district.id, "address.district", "District")?;
        require_positive(info.address.province.id, "address.province", "Province")?;
        require_positive(
            info.address.postal_code.id,
            "address.postal_code",
            "Postal code",
        )?;
        require_positive(
            info.education.medical_school.id,
            "education.medical_school",
            "Medical school",
        )?;
        Ok(())
    }

    fn validate_collections(info: &OnBoardingStub) -> AppResult<()> {
        if !info.work_place.primary.iter().any(|w| w.id.is_positive()) {
            return Err(validation_error(
                "work_place.primary",
                "At least one primary workplace is required",
            ));
        }
        require_positive(
            info.education.specialties.id,
            "education.specialties",
            "Specialty",
        )?;
        if info.documents.certificate_image_urls.len() > 6 {
            return Err(validation_error(
                "documents.certificate_image_urls",
                "Certificate images cannot exceed 6",
            ));
        }
        Ok(())
    }

    fn validate_documents(info: &OnBoardingStub) -> AppResult<()> {
        let docs = &info.documents;
        if docs.profile_image_url.is_empty() {
            return Err(validation_error(
                "profile_image_url",
                "profile_image_url is required",
            ));
        }
        if docs.id_card_image_url.is_empty() {
            return Err(validation_error(
                "id_card_image_url",
                "id_card_image_url is required",
            ));
        }
        if docs.book_bank_image_url.is_empty() {
            return Err(validation_error(
                "book_bank_image_url",
                "book_bank_image_url is required",
            ));
        }
        if docs.med_license_image_url.is_empty() {
            return Err(validation_error(
                "med_license_image_url",
                "med_license_image_url is required",
            ));
        }
        Ok(())
    }

    pub fn validate_onboarding_submission(&self, info: &OnBoardingStub) -> AppResult<()> {
        Self::validate_text_fields(info)?;
        Self::validate_required_ids(info)?;
        Self::validate_collections(info)?;
        Self::validate_documents(info)?;
        Ok(())
    }

    pub fn validate_onboarding_status_transition(
        &self,
        current_status: &OnBoardingStatus,
        new_status: &OnBoardingStatus,
    ) -> AppResult<()> {
        match (current_status, new_status) {
            (OnBoardingStatus::Draft, OnBoardingStatus::PendingApproval) => Ok(()),
            (OnBoardingStatus::Rejected { .. }, OnBoardingStatus::PendingApproval) => Ok(()),
            (OnBoardingStatus::PendingApproval, OnBoardingStatus::Approved) => Ok(()),
            (OnBoardingStatus::PendingApproval, OnBoardingStatus::Rejected { .. }) => Ok(()),
            (_, OnBoardingStatus::CancelledByUser) => Ok(()),
            (_, OnBoardingStatus::Deactivated { .. }) => Ok(()),
            _ => Err(AppError::BadRequest(format!(
                "Invalid onboarding status transition from {:?} to {:?}",
                current_status, new_status
            ))),
        }
    }
}

impl Default for OnboardingValidatorImp {
    fn default() -> Self {
        Self::new()
    }
}

fn require_positive(id: i32, field: &str, label: &str) -> AppResult<()> {
    if id.is_positive() {
        Ok(())
    } else {
        Err(validation_error(field, &format!("{label} is required")))
    }
}

fn require_non_empty(value: &str, field: &str, label: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        Err(validation_error(field, &format!("{label} is required")))
    } else {
        Ok(())
    }
}

pub fn validate_thai_citizen_id(citizen_id: &str) -> AppResult<()> {
    if citizen_id.len() != 13 || !citizen_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(validation_error(
            "citizen_id",
            "Citizen ID must be exactly 13 digits",
        ));
    }
    Ok(())
}

fn validation_error(field: &str, message: &str) -> AppError {
    AppError::ValidationFieldError {
        field: field.to_string(),
        message: message.to_string(),
    }
}
