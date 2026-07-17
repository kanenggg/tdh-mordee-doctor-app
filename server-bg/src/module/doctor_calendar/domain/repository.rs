use async_trait::async_trait;

use common::core::error::{AppError, AppResult};
use common::repo::firebase_repo::FirebaseRepo;

use super::models::RtdbAppointment;

/// Trait for consultation event repository backed by Firebase RTDB
#[async_trait]
pub trait RtdbConsultationEventRepoTrait: Send + Sync {
    async fn get_appointment_state(
        &self,
        booking_id: &str,
        doctor_id: i32,
        date: &str,
    ) -> AppResult<Option<RtdbAppointment>>;

    async fn upsert_appointment_state(
        &self,
        booking_id: &str,
        doctor_id: i32,
        date: &str,
        appointment: &RtdbAppointment,
    ) -> AppResult<()>;

    async fn delete_appointment_state(
        &self,
        booking_id: &str,
        doctor_id: i32,
        date_hint: &str,
    ) -> AppResult<bool>;
}

pub struct RtdbConsultationEventRepo {
    firebase: FirebaseRepo,
    collection: String,
}

impl RtdbConsultationEventRepo {
    pub fn new(firebase: FirebaseRepo, collection: String) -> Self {
        Self {
            firebase,
            collection,
        }
    }

    fn appointment_path(&self, doctor_id: i32, date: &str, booking_id: &str) -> String {
        format!("{}/{}/{}/{}", self.collection, doctor_id, date, booking_id)
    }
}

#[async_trait]
impl RtdbConsultationEventRepoTrait for RtdbConsultationEventRepo {
    async fn get_appointment_state(
        &self,
        booking_id: &str,
        doctor_id: i32,
        date: &str,
    ) -> AppResult<Option<RtdbAppointment>> {
        let path = self.appointment_path(doctor_id, date, booking_id);
        let value = self.firebase.get(&path).await?;
        match value {
            Some(v) => {
                let appointment: RtdbAppointment = serde_json::from_value(v).map_err(|e| {
                    AppError::InternalError(format!(
                        "Failed to deserialize RTDB appointment: {}",
                        e
                    ))
                })?;
                Ok(Some(appointment))
            }
            None => Ok(None),
        }
    }

    async fn upsert_appointment_state(
        &self,
        booking_id: &str,
        doctor_id: i32,
        date: &str,
        appointment: &RtdbAppointment,
    ) -> AppResult<()> {
        let path = self.appointment_path(doctor_id, date, booking_id);
        let value = serde_json::to_value(appointment).map_err(|e| {
            AppError::InternalError(format!("Failed to serialize appointment: {}", e))
        })?;
        self.firebase.update(&path, &value).await
    }

    async fn delete_appointment_state(
        &self,
        booking_id: &str,
        doctor_id: i32,
        date_hint: &str,
    ) -> AppResult<bool> {
        let exact_path = self.appointment_path(doctor_id, date_hint, booking_id);
        if self.firebase.exists(&exact_path).await? {
            self.firebase
                .set(&exact_path, &serde_json::Value::Null)
                .await?;
            return Ok(true);
        }

        let doctor_path = format!("{}/{}", self.collection, doctor_id);
        let Some(serde_json::Value::Object(dates)) = self.firebase.get(&doctor_path).await? else {
            return Ok(false);
        };

        for (date, bookings) in dates {
            if bookings.get(booking_id).is_some() {
                let path = self.appointment_path(doctor_id, &date, booking_id);
                self.firebase.set(&path, &serde_json::Value::Null).await?;
                return Ok(true);
            }
        }

        Ok(false)
    }
}
