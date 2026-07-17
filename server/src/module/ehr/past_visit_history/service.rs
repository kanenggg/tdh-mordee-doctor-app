use std::sync::Arc;

use crate::core::error::AppResult;

use super::gateway::{ApmAppointment, PastVisitGateway, PastVisitsFromGateway};
use super::handler::{GetPastVisitHistoryResult, PastVisit};

#[derive(Clone)]
pub struct PastVisitHistoryService {
    gateway: Arc<PastVisitGateway>,
}

impl PastVisitHistoryService {
    pub fn new(gateway: Arc<PastVisitGateway>) -> Self {
        Self { gateway }
    }

    pub async fn get_past_visits(
        &self,
        request_id: &str,
        patient_account_id: i32,
        patient_profile_id: i32,
    ) -> AppResult<GetPastVisitHistoryResult> {
        let outcome = self
            .gateway
            .get_past_visits(request_id, patient_account_id, patient_profile_id)
            .await?;

        Ok(match outcome {
            PastVisitsFromGateway::Found(appointments) => GetPastVisitHistoryResult::PastVisits {
                past_visits: appointments.into_iter().map(map_appointment).collect(),
            },
            PastVisitsFromGateway::NotFound => GetPastVisitHistoryResult::NotFound,
            PastVisitsFromGateway::Unauthorized => GetPastVisitHistoryResult::Unauthorized,
        })
    }
}

// Convert biz-apm's native appointment (nested time + embedded doctor) into the
// flat past-visit response shape.
fn map_appointment(a: ApmAppointment) -> PastVisit {
    PastVisit::new(
        a.booking_id,
        a.appointment_time.start_time,
        a.appointment_time.end_time,
        format!("{} {}", a.doctor.first_name, a.doctor.last_name),
    )
}
