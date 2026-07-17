use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::core::error::{AppError, AppResult};

use super::super::models::{DrugAllergyInfo, PrescriptionItem};
use super::external_http_client::{CreatedPrescription, JadeServiceTrait};

// ─── Request types (POST /prescription/create) ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrescriptionRequest {
    booking_id: String,
    biz_unit_id: i32,
    biz_center_id: i32,
    doctor_id: String,
    patient_profile_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    prescription_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prescription_expired_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    acknowledge_allergy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allergies: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<Vec<PrescriptionItem>>,
}

// ─── Response types (POST /prescription/create) ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type")]
pub enum CreatePrescriptionResponse {
    #[serde(rename = "Success")]
    #[serde(rename_all = "camelCase")]
    Success {
        prescription_id: i32,
        prescription_code: String,
    },
}

pub struct JadeHttpClient {
    client: Client,
    base_uri: String,
}

impl JadeHttpClient {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client for Jade service"),
            base_uri,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_prescription_request(
    appointment_id: &str,
    items: &[PrescriptionItem],
    biz_unit_id: i32,
    biz_center_id: i32,
    doctor_id: &str,
    patient_profile_id: i32,
    prescription_expired_at: Option<i64>,
    drug_allergy_info: Option<&DrugAllergyInfo>,
) -> PrescriptionRequest {
    let allergies = match drug_allergy_info {
        Some(DrugAllergyInfo::HasDrugAllergies { drug_allergies }) => {
            Some(drug_allergies.iter().map(|a| a.id).collect())
        }
        Some(DrugAllergyInfo::NoDrugAllergies) | None => None,
    };

    PrescriptionRequest {
        booking_id: appointment_id.to_string(),
        biz_unit_id,
        biz_center_id,
        doctor_id: doctor_id.to_string(),
        patient_profile_id,
        prescription_code: None,
        prescription_expired_at,
        acknowledge_allergy: Some(true),
        allergies,
        items: Some(items.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::models::{DrugAllergy, RefData, RefDataWithAmount};
    use super::*;

    fn sample_item() -> PrescriptionItem {
        PrescriptionItem {
            price_plan_id: 101,
            medicine_id: 101,
            medicine_name: "Paracetamol 500mg".to_string(),
            dose: RefDataWithAmount {
                id: 1,
                value: 500,
                unit: "mg".to_string(),
            },
            quantity: 10,
            route: RefData {
                id: 1,
                description: "Oral".to_string(),
            },
            frequency: RefData {
                id: 2,
                description: "3 times a day".to_string(),
            },
            indication: RefData {
                id: 3,
                description: "Pain relief".to_string(),
            },
            meal_instruction: RefData {
                id: 5,
                description: "Take with food".to_string(),
            },
            duration: RefDataWithAmount {
                id: 99,
                value: 7,
                unit: "days".to_string(),
            },
            cautions: None,
            remark: None,
            note_to_patient: None,
            unit_price: 125.0,
        }
    }

    #[test]
    fn prescription_request_maps_item_fields_from_domain_model() {
        let request = build_prescription_request(
            "APT-001",
            &[sample_item()],
            11,
            22,
            "doctor-uuid-33",
            44,
            Some(1_718_035_200),
            None,
        );

        let json = serde_json::to_value(request).unwrap();
        assert_eq!(json["prescriptionExpiredAt"], 1_718_035_200);
        assert_eq!(json["acknowledgeAllergy"], true);
        assert!(json.get("allergies").is_none());

        let item = &json["items"][0];
        assert_eq!(item["medicineId"], 101);
        assert_eq!(item["medicineName"], "Paracetamol 500mg");
        assert_eq!(item["pricePlanId"], 101);
        assert_eq!(item["unitPrice"], 125.0);
        assert_eq!(item["quantity"], 10);
        assert_eq!(
            item["dose"],
            serde_json::json!({ "id": 1, "value": 500, "unit": "mg" })
        );
        assert_eq!(
            item["duration"],
            serde_json::json!({ "id": 99, "value": 7, "unit": "days" })
        );
        assert_eq!(
            item["route"],
            serde_json::json!({ "id": 1, "description": "Oral" })
        );
        assert_eq!(
            item["frequency"],
            serde_json::json!({ "id": 2, "description": "3 times a day" })
        );
        assert_eq!(
            item["indication"],
            serde_json::json!({ "id": 3, "description": "Pain relief" })
        );
        assert_eq!(
            item["mealInstruction"],
            serde_json::json!({ "id": 5, "description": "Take with food" })
        );
    }

    #[test]
    fn prescription_request_forwards_allergy_ids_when_present() {
        let allergy_info = DrugAllergyInfo::HasDrugAllergies {
            drug_allergies: vec![
                DrugAllergy {
                    id: 7,
                    display_text: "Penicillin".to_string(),
                },
                DrugAllergy {
                    id: 9,
                    display_text: "Aspirin".to_string(),
                },
            ],
        };

        let request = build_prescription_request(
            "APT-001",
            &[sample_item()],
            11,
            22,
            "doctor-uuid-33",
            44,
            None,
            Some(&allergy_info),
        );

        let json = serde_json::to_value(request).unwrap();
        assert_eq!(json["acknowledgeAllergy"], true);
        assert_eq!(json["allergies"], serde_json::json!([7, 9]));
    }

    #[test]
    fn prescription_request_acknowledges_allergy_with_no_allergy_list() {
        for info in [None, Some(&DrugAllergyInfo::NoDrugAllergies)] {
            let request = build_prescription_request(
                "APT-001",
                &[sample_item()],
                11,
                22,
                "doctor-uuid-33",
                44,
                None,
                info,
            );

            let json = serde_json::to_value(request).unwrap();
            assert_eq!(json["acknowledgeAllergy"], true);
            assert!(json.get("allergies").is_none());
        }
    }
}

#[async_trait]
impl JadeServiceTrait for JadeHttpClient {
    async fn create_prescription(
        &self,
        request_id: &str,
        appointment_id: &str,
        items: &[PrescriptionItem],
        biz_unit_id: i32,
        biz_center_id: i32,
        doctor_id: &str,
        patient_profile_id: i32,
        prescription_expired_at: Option<i64>,
        drug_allergy_info: Option<&DrugAllergyInfo>,
    ) -> AppResult<CreatedPrescription> {
        let url = format!("{}/prescription/create", self.base_uri);

        let body = build_prescription_request(
            appointment_id,
            items,
            biz_unit_id,
            biz_center_id,
            doctor_id,
            patient_profile_id,
            prescription_expired_at,
            drug_allergy_info,
        );

        let has_drug_allergies = match drug_allergy_info {
            Some(DrugAllergyInfo::HasDrugAllergies { .. }) => true,
            Some(DrugAllergyInfo::NoDrugAllergies) | None => false,
        };

        info!(
            request_id = %request_id,
            appointment_id = %appointment_id,
            url = %url,
            "Calling Jade service to create prescription"
        );
        debug!(
            request_id = %request_id,
            item_count = items.len(),
            has_drug_allergies,
            has_prescription_expiry = prescription_expired_at.is_some(),
            "Prepared Jade prescription request"
        );

        let resp = self
            .client
            .post(&url)
            .header("X-Request-Id", request_id)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                warn!(
                    request_id = %request_id,
                    error = %e,
                    "Failed to call Jade prescription service"
                );
                AppError::InternalError(format!("Jade service request failed: {}", e))
            })?;

        let status = resp.status();
        let response_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        debug!(
            request_id = %request_id,
            status = %status,
            response_bytes = response_text.len(),
            "Jade response received"
        );

        if !status.is_success() {
            warn!(
                request_id = %request_id,
                status = %status,
                response_bytes = response_text.len(),
                "Jade prescription service returned error"
            );
            return Err(AppError::InternalError(format!(
                "Jade service returned status {}",
                status
            )));
        }

        let response: CreatePrescriptionResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                warn!(
                    request_id = %request_id,
                    error = %e,
                    response_bytes = response_text.len(),
                    "Failed to deserialize Jade response"
                );
                AppError::InternalError(format!("Failed to parse Jade response: {}", e))
            })?;

        match response {
            CreatePrescriptionResponse::Success {
                prescription_id,
                prescription_code,
            } => {
                info!(
                    request_id = %request_id,
                    appointment_id = %appointment_id,
                    prescription_code = %prescription_code,
                    prescription_id = prescription_id,
                    "Prescription created successfully"
                );
                Ok(CreatedPrescription {
                    id: i64::from(prescription_id),
                    code: prescription_code,
                })
            }
        }
    }
}
