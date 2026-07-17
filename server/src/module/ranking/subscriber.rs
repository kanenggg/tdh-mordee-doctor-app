use serde::Deserialize;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use super::cache::RankingCacheTrait;
use super::repo::RankingRepoTrait;

/// Event payload for doctor availability changes (from Pub/Sub).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorAvailabilityEvent {
    pub event_type: String,
    pub doctor_id: Uuid,
    #[serde(default)]
    pub instant_mode_enabled: Option<bool>,
    #[serde(default)]
    pub schedule_mode_enabled: Option<bool>,
}

/// Process a doctor availability change event.
pub async fn handle_availability_event(
    event: &DoctorAvailabilityEvent,
    repo: &Arc<dyn RankingRepoTrait>,
    cache: &Arc<dyn RankingCacheTrait>,
) {
    let doctor_id = event.doctor_id;

    // Fetch current score from DB
    let score = match repo.get_doctor_score(doctor_id).await {
        Ok(Some(s)) => s as f64,
        Ok(None) => {
            warn!(doctor_id = %doctor_id, "Doctor score not found for availability update");
            0.0
        }
        Err(e) => {
            warn!(doctor_id = %doctor_id, error = %e, "Failed to fetch doctor score for availability update");
            return;
        }
    };

    if let Some(instant) = event.instant_mode_enabled {
        if instant {
            cache.add_to_instant(doctor_id, score).await;
        } else {
            cache.remove_from_instant(doctor_id).await;
        }
    }

    if let Some(scheduled) = event.schedule_mode_enabled {
        if scheduled {
            cache.add_to_scheduled(doctor_id, score).await;
        } else {
            cache.remove_from_scheduled(doctor_id).await;
        }
    }

    cache.invalidate_profile(doctor_id).await;

    info!(
        doctor_id = %doctor_id,
        "Processed availability event"
    );
}

/// Handle consultation session start — remove doctor from instant ranking.
pub async fn handle_session_started(doctor_id: Uuid, cache: &Arc<dyn RankingCacheTrait>) {
    cache.remove_from_instant(doctor_id).await;
    cache.invalidate_profile(doctor_id).await;
    info!(doctor_id = %doctor_id, "Doctor removed from instant ranking (session started)");
}

/// Handle consultation session end — re-add doctor to instant ranking.
pub async fn handle_session_ended(doctor_id: Uuid, score: f64, cache: &Arc<dyn RankingCacheTrait>) {
    cache.add_to_instant(doctor_id, score).await;
    cache.invalidate_profile(doctor_id).await;
    info!(doctor_id = %doctor_id, "Doctor re-added to instant ranking (session ended)");
}
