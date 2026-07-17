use crate::{
    appointment::v2::payment_transaction::PaymentChannels,
    common::PartialUserIdentity,
    consultation::{
        consultation_pre_screen::ConsultationPreScreen, BookingType, ConsultationChannel,
    },
};

fn default_consult_duration() -> Option<i32> {
    Some(20)
}
fn default_booking_type() -> BookingType {
    BookingType::Instant
}

fn default_consultation_channel() -> ConsultationChannel {
    ConsultationChannel::Video
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConfirmedInstantAppointmentRequest {
    pub biz_unit_id: i32,
    pub biz_center_id: i32,
    pub tenant_id: i32,
    pub patient_id: PartialUserIdentity,
    pub doctor_id: PartialUserIdentity,
    pub prescreen: ConsultationPreScreen,
    #[serde(
        skip_serializing_if = "Option::is_none",
        default = "default_consult_duration"
    )]
    pub consult_duration: Option<i32>,
    #[serde(default = "default_booking_type")]
    pub booking_type: BookingType,
    #[serde(default = "default_consultation_channel")]
    pub consultation_channel: ConsultationChannel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_appointment_id: Option<String>,
    pub payment_channels: PaymentChannels,
}
