use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(bound = ""))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookingRequest<A = Vec<u8>> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_number: Option<String>,
    pub meet_type: String,
    pub doctor_usid: String,
    pub schedule_id: i64,
    pub duration_of_symptom: i32,
    pub allergies: Vec<String>,
    pub symptom: String,
    pub duration_unit: i32,
    pub meet_channel: String,
    #[cfg_attr(feature = "openapi", schema(value_type = Vec<String>, format = Binary))]
    pub attachments: Vec<A>,
}

/// Example
///{
///  "message": "Success",
///  "code": "SUCCESS",
///  "returnType": "",
///  "data": {
///    "id": 19908,
///    "orderStatus": "draft",
///    "orderTotal": 375,
///    "orderActualTotal": 0,
///    "orderDiscount": 0,
///    "orderVat": 0,
///    "orderSubTotal": 0,
///    "orderGrandTotal": 375,
///    "invoiceNumber": "",
///    "orderNumber": "20260112034281",
///    "receiptNumber": "260112G5QX7N",
///    "isActive": true,
///    "isDeleted": false,
///    "createdAt": "2026-01-12T08:15:58.455792181Z",
///    "updatedAt": "2026-01-12T08:15:58.455792181Z",
///    "createdBy": "",
///    "updatedBy": "",
///    "parentOrderID": 0,
///    "accountUsid": "32db206b-9915-4b99-bc4b-56a5d9d67ac9",
///    "clientID": "dd88c63b-df64-48b9-8d2a-9faac1a6c43e",
///    "orderItems": null,
///    "paymentTransactionID": "",
///    "insurancePolicyID": 0,
///    "paymentExpiresAt": "0001-01-01T00:00:00Z",
///    "contactEmail": "",
///    "sendConfirmPharmacy": false
///  }
///}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BookingResponse {
    pub id: i64,
    pub order_status: String,
    pub order_total: i64,
    pub order_actual_total: i64,
    pub order_discount: i64,
    pub order_vat: i64,
    pub order_sub_total: i64,
    pub order_grand_total: i64,
    pub invoice_number: String,
    pub order_number: String,
    pub receipt_number: String,
    pub is_active: bool,
    pub is_deleted: bool,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: String,
    pub updated_by: String,
    #[serde(rename = "parentOrderID")]
    pub parent_order_id: i64,
    pub account_usid: String,
    #[serde(rename = "clientID")]
    pub client_id: String,
    pub order_items: Option<Vec<serde_json::Value>>,
    #[serde(rename = "paymentTransactionID")]
    pub payment_transaction_id: String,
    #[serde(rename = "insurancePolicyID")]
    pub insurance_policy_id: i64,
    pub payment_expires_at: String,
    pub contact_email: String,
    pub send_confirm_pharmacy: bool,
}
