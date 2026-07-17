use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::model::ref_data::{
    AcademicPosition, District, MedicalSchool, PostalCode, Profession, Province, SubDistrict,
    WorkPlace,
};
use tdh_protocol::common::Localized;

// ============================================================================
// Request (input) types — used for POST /onboarding/v1
// serde ignores unknown fields by default, so the frontend can send extra
// fields like `description` or `name` and they are silently discarded.
// ============================================================================

/// Thin ID-only wrapper for any reference-data object.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RefId {
    pub id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnBoardingRequest {
    pub profession: Profession,
    pub academic_position: AcademicPosition,
    pub citizen_id: String,
    pub first_name: Localized,
    pub last_name: Localized,
    pub address: Address,
    pub work_place: SelectedWorkPlaceRequest,
    pub education: EducationRequest,
    pub documents: Documents,
}

impl Default for OnBoardingRequest {
    fn default() -> Self {
        Self {
            profession: Profession::default(),
            academic_position: AcademicPosition::default(),
            citizen_id: String::new(),
            first_name: Localized {
                th: String::new(),
                en: String::new(),
            },
            last_name: Localized {
                th: String::new(),
                en: String::new(),
            },
            address: Address::default(),
            work_place: SelectedWorkPlaceRequest::default(),
            education: EducationRequest::default(),
            documents: Documents::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnBoardingRequestPatch {
    pub profession: Option<Profession>,
    pub academic_position: Option<AcademicPosition>,
    pub citizen_id: Option<String>,
    pub first_name: Option<Localized>,
    pub last_name: Option<Localized>,
    pub address: Option<Address>,
    pub work_place: Option<SelectedWorkPlaceRequest>,
    pub education: Option<EducationRequest>,
    pub documents: Option<Documents>,
}

impl OnBoardingRequest {
    pub fn apply(&mut self, patch: OnBoardingRequestPatch) {
        if let Some(v) = patch.profession {
            self.profession = v;
        }
        if let Some(v) = patch.academic_position {
            self.academic_position = v;
        }
        if let Some(v) = patch.citizen_id {
            self.citizen_id = v;
        }
        if let Some(v) = patch.first_name {
            self.first_name = v;
        }
        if let Some(v) = patch.last_name {
            self.last_name = v;
        }
        if let Some(v) = patch.address {
            self.address = v;
        }
        if let Some(v) = patch.work_place {
            self.work_place = v;
        }
        if let Some(v) = patch.education {
            self.education = v;
        }
        if let Some(v) = patch.documents {
            self.documents = v;
        }
    }
}

impl PartialEq for OnBoardingRequest {
    fn eq(&self, other: &Self) -> bool {
        self.profession == other.profession
            && self.academic_position == other.academic_position
            && self.citizen_id == other.citizen_id
            && self.first_name.th == other.first_name.th
            && self.first_name.en == other.first_name.en
            && self.last_name.th == other.last_name.th
            && self.last_name.en == other.last_name.en
            && self.address == other.address
            && self.work_place == other.work_place
            && self.education == other.education
            && self.documents == other.documents
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SelectedWorkPlaceRequest {
    pub primary: Vec<WorkPlace>,
    pub additional: Vec<WorkPlace>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EducationRequest {
    pub license_number: String,
    pub medical_school: MedicalSchool,
    pub specialties: Specialty,
    pub additional_specialties: Vec<Specialty>,
    pub special_interests: Vec<String>,
}

/// Bridges OnBoardingRequest (HTTP-layer input) to OnBoardingStub (validator input).
/// Used by the submit flow to validate a patch-resolved request without a DB round-trip,
/// and by tests to construct stubs directly. Seeds status as Draft — the actual submitted
/// status (PendingApproval) is set atomically by the SQL function, not via this conversion.
impl From<OnBoardingRequest> for OnBoardingStub {
    fn from(input: OnBoardingRequest) -> Self {
        OnBoardingStub {
            profession: input.profession,
            academic_position: input.academic_position,
            citizen_id: input.citizen_id,
            first_name: input.first_name,
            last_name: input.last_name,
            address: input.address,
            work_place: SelectedWorkPlace {
                primary: input.work_place.primary,
                additional: input.work_place.additional,
            },
            education: Education {
                license_number: input.education.license_number,
                medical_school: input.education.medical_school,
                specialties: input.education.specialties,
                additional_specialties: input.education.additional_specialties,
                special_interests: input.education.special_interests,
            },
            documents: input.documents,
            status: OnBoardingStatus::Draft,
        }
    }
}

// ============================================================================
// Domain types — two explicit states:
//
//   OnBoardingStub  — raw data from the database; reference-data descriptions
//                     are empty strings. Cannot be serialized (no Serialize).
//                     Used by the repo, the validator, and submit_doctor_profile_draft.
//
//   OnBoarding      — fully enriched; all descriptions populated by the lookup
//                     service. Carries Serialize/ToSchema for HTTP responses.
//                     Produced only by OnboardingService::get_onboarding.
//
// The type split prevents a future code path from accidentally serializing
// a stub (empty descriptions) and sending it to clients.
// ============================================================================

/// Raw onboarding data as read from the database.
/// All reference-data description fields are empty strings.
/// To get a client-ready value, pass through OnboardingService::get_onboarding.
#[derive(Debug, Clone)]
pub struct OnBoardingStub {
    pub citizen_id: String,
    pub profession: Profession,
    pub academic_position: AcademicPosition,
    pub first_name: Localized,
    pub last_name: Localized,
    pub address: Address,
    pub work_place: SelectedWorkPlace,
    pub education: Education,
    pub documents: Documents,
    pub status: OnBoardingStatus,
}

impl From<OnBoardingStub> for OnBoarding {
    fn from(stub: OnBoardingStub) -> Self {
        OnBoarding {
            profession: stub.profession,
            academic_position: stub.academic_position,
            citizen_id: stub.citizen_id,
            first_name: stub.first_name,
            last_name: stub.last_name,
            address: stub.address,
            work_place: stub.work_place,
            education: stub.education,
            documents: stub.documents,
            status: stub.status,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnBoarding {
    pub profession: Profession,
    pub academic_position: AcademicPosition,
    pub citizen_id: String,
    pub first_name: Localized,
    pub last_name: Localized,
    pub address: Address,
    pub work_place: SelectedWorkPlace,
    pub education: Education,
    pub documents: Documents,
    pub status: OnBoardingStatus,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum OnBoardingStatus {
    #[default]
    Draft,
    PendingApproval,
    CancelledByUser,
    Approved,
    Rejected {
        reason: String,
    },
    Deactivated {
        reason: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub address_detail: String,
    pub sub_district: SubDistrict,
    pub district: District,
    pub province: Province,
    pub postal_code: PostalCode,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SelectedWorkPlace {
    pub primary: Vec<WorkPlace>,
    pub additional: Vec<WorkPlace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Specialty {
    pub id: i32,
    pub name: Localized,
    pub subspecialty: Subspecialty,
    pub medical_school: MedicalSchool,
}

impl Default for Specialty {
    fn default() -> Self {
        Self {
            id: 0,
            name: Localized {
                th: String::new(),
                en: String::new(),
            },
            subspecialty: Subspecialty::default(),
            medical_school: MedicalSchool::default(),
        }
    }
}

impl PartialEq for Specialty {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name.th == other.name.th
            && self.name.en == other.name.en
            && self.subspecialty == other.subspecialty
            && self.medical_school == other.medical_school
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Subspecialty {
    pub id: i32,
    pub name: Localized,
    pub medical_school: MedicalSchool,
}

impl Default for Subspecialty {
    fn default() -> Self {
        Self {
            id: 0,
            name: Localized {
                th: String::new(),
                en: String::new(),
            },
            medical_school: MedicalSchool::default(),
        }
    }
}

impl PartialEq for Subspecialty {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name.th == other.name.th
            && self.name.en == other.name.en
            && self.medical_school == other.medical_school
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Education {
    pub license_number: String,
    pub medical_school: MedicalSchool,
    pub specialties: Specialty,
    pub additional_specialties: Vec<Specialty>,
    pub special_interests: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Documents {
    pub profile_image_url: String,
    pub id_card_image_url: String,
    pub book_bank_image_url: String,
    pub med_license_image_url: String,
    pub certificate_image_urls: Vec<String>,
}
