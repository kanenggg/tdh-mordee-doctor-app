use crate::model::onboarding::RefId;
use serde::{Deserialize, Serialize};
use tdh_protocol::common::Localized;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Profession {
    pub id: i32,
    pub name: Localized,
    pub abbr: Localized,
}

impl Default for Profession {
    fn default() -> Self {
        Self {
            id: 0,
            name: Localized {
                th: String::new(),
                en: String::new(),
            },
            abbr: Localized {
                th: String::new(),
                en: String::new(),
            },
        }
    }
}

impl PartialEq for Profession {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name.th == other.name.th
            && self.name.en == other.name.en
            && self.abbr.th == other.abbr.th
            && self.abbr.en == other.abbr.en
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicPosition {
    pub id: i32,
    pub name: Localized,
    pub abbr: Localized,
}

impl Default for AcademicPosition {
    fn default() -> Self {
        Self {
            id: 0,
            name: Localized {
                th: String::new(),
                en: String::new(),
            },
            abbr: Localized {
                th: String::new(),
                en: String::new(),
            },
        }
    }
}

impl PartialEq for AcademicPosition {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name.th == other.name.th
            && self.name.en == other.name.en
            && self.abbr.th == other.abbr.th
            && self.abbr.en == other.abbr.en
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubDistrict {
    pub id: i32,
    pub name: Localized,
    pub district_id: i32,
    pub zip_code: String,
}

impl Default for SubDistrict {
    fn default() -> Self {
        Self {
            id: 0,
            name: Localized {
                th: String::new(),
                en: String::new(),
            },
            district_id: 0,
            zip_code: String::new(),
        }
    }
}

impl PartialEq for SubDistrict {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name.th == other.name.th
            && self.name.en == other.name.en
            && self.district_id == other.district_id
            && self.zip_code == other.zip_code
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct District {
    pub id: i32,
    pub name: Localized,
    pub province_id: i32,
}

impl Default for District {
    fn default() -> Self {
        Self {
            id: 0,
            name: Localized {
                th: String::new(),
                en: String::new(),
            },
            province_id: 0,
        }
    }
}

impl PartialEq for District {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name.th == other.name.th
            && self.name.en == other.name.en
            && self.province_id == other.province_id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Province {
    pub id: i32,
    pub name: Localized,
}

impl Default for Province {
    fn default() -> Self {
        Self {
            id: 0,
            name: Localized {
                th: String::new(),
                en: String::new(),
            },
        }
    }
}

impl PartialEq for Province {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.name.th == other.name.th && self.name.en == other.name.en
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PostalCode {
    pub id: i32,
    pub district_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkPlace {
    pub id: i32,
    pub name: Localized,
}

impl Default for WorkPlace {
    fn default() -> Self {
        Self {
            id: 0,
            name: Localized {
                th: String::new(),
                en: String::new(),
            },
        }
    }
}

impl PartialEq for WorkPlace {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.name.th == other.name.th && self.name.en == other.name.en
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MedicalSchool {
    pub id: i32,
    pub name: Localized,
}

impl Default for MedicalSchool {
    fn default() -> Self {
        Self {
            id: 0,
            name: Localized {
                th: String::new(),
                en: String::new(),
            },
        }
    }
}

impl PartialEq for MedicalSchool {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.name.th == other.name.th && self.name.en == other.name.en
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Specialty {
    pub id: i32,
    pub subspecialty: Subspecialty,
    pub medical_school: RefId,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub address_detail: String,
    pub sub_district: SubDistrict,
    pub district: District,
    pub province: Province,
    pub postal_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Subspecialty {
    pub id: i32,
    pub medical_school: RefId,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Icd10 {
    pub code: String,
    pub description: String,
}
