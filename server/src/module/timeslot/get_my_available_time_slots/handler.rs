use axum::extract::{Query, State};
use axum::Json;
use jiff::civil::{Date, Time};
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};

use crate::core::auth::DoctorIdentity;
use crate::core::error::{AppError, AppResult};
use crate::module::timeslot::common::generate_full_day_timeslots;
use crate::module::timeslot::handler::{MyAvailableQuery, MyAvailableResponse, TimeslotState};

/// Consultation duration → slot block size: add the rest gap.
/// 15→20, 25→30, 50→60; any other value passes through unchanged.
pub fn gap_rule(duration: i32) -> i32 {
    match duration {
        15 => 20,
        25 => 30,
        50 => 60,
        other => other,
    }
}

/// Accept either a civil date (`2026-04-02`) or a full datetime such as
/// `2026-04-02T00:00:00+07:00` (the `+` may arrive URL-decoded to a space) by
/// falling back to the leading `YYYY-MM-DD`. Only the calendar date is used;
/// the window's timezone comes from the `time_zone` query param.
fn parse_query_date(raw: &str) -> Option<Date> {
    raw.parse().ok().or_else(|| raw.get(..10)?.parse().ok())
}

#[utoipa::path(
    get,
    path = "/timeslot/v1/me/available-timeslots",
    tag = "timeslot",
    params(crate::module::timeslot::handler::MyAvailableQuery),
    responses(
        (status = 200, description = "Doctor's available timeslots for the date", body = crate::module::timeslot::handler::MyAvailableResponse),
        (status = 400, description = "Invalid date format"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - not a doctor account"),
    )
)]
pub async fn get_my_available_timeslots(
    State(state): State<TimeslotState>,
    identity: DoctorIdentity,
    Query(query): Query<MyAvailableQuery>,
) -> AppResult<Json<MyAvailableResponse>> {
    let date: Date = parse_query_date(&query.date).ok_or_else(|| {
        AppError::BadRequest("Invalid date format, expected a date like 2026-04-02".to_string())
    })?;
    let tz = TimeZone::get(&query.time_zone)
        .map_err(|_| AppError::BadRequest(format!("Invalid timezone: {}", query.time_zone)))?;

    // Day bounds in the requested tz, sent upstream as UTC RFC-3339 (Z) so the
    // query string carries no `+` offset to misencode.
    let start = date
        .at(0, 0, 0, 0)
        .to_zoned(tz.clone())
        .map_err(|e| AppError::InternalError(format!("zoning failed: {}", e)))?;
    let end = start
        .checked_add(1.day())
        .map_err(|e| AppError::InternalError(format!("zoning failed: {}", e)))?;
    let from_datetime = start.timestamp().to_string();
    let to_datetime = end.timestamp().to_string();

    // DoctorService reads its own consultation-duration config (self-contained repo).
    let slot_duration = match state
        .consultation_duration_repo
        .get_consultation_duration(identity.doctor_profile_id)
        .await
        .map_err(|e| {
            AppError::InternalError(format!("Failed to get consultation duration: {}", e))
        })? {
        Some(duration) => duration,
        // No consultation-duration config for this doctor → no schedule.
        None => return Ok(Json(MyAvailableResponse::NoScheduleConfig)),
    };

    // Reserved slots come from ConsultationService (epoch seconds, UTC).
    let reserved = state
        .reserved_timeslots_client
        .get_reserved_timeslots(identity.doctor_profile_id, &from_datetime, &to_datetime)
        .await?;

    // Convert each reserved epoch range to the requested timezone's civil (start, end) times.
    let reserved_civil: Vec<(Time, Time)> = reserved
        .iter()
        .map(|r| {
            let s = Timestamp::from_second(r.start_time)
                .map_err(|e| AppError::InternalError(format!("bad start_time: {}", e)))?
                .to_zoned(tz.clone())
                .time();
            let e = Timestamp::from_second(r.end_time)
                .map_err(|e| AppError::InternalError(format!("bad end_time: {}", e)))?
                .to_zoned(tz.clone())
                .time();
            Ok::<(Time, Time), AppError>((s, e))
        })
        .collect::<Result<_, _>>()?;

    let block = gap_rule(slot_duration);
    let timeslots = generate_full_day_timeslots(date, block, &reserved_civil)
        .into_iter()
        .map(Into::into)
        .collect();

    Ok(Json(MyAvailableResponse::Success { timeslots }))
}

#[cfg(test)]
mod gap_rule_tests {
    use super::{gap_rule, parse_query_date};

    #[test]
    fn applies_known_gaps_and_passes_through_unknown() {
        assert_eq!(gap_rule(15), 20);
        assert_eq!(gap_rule(25), 30);
        assert_eq!(gap_rule(50), 60);
        assert_eq!(gap_rule(40), 40); // unknown → passthrough
    }

    #[test]
    fn parse_query_date_accepts_plain_and_datetime() {
        let expected = "2026-04-02".parse().ok();
        assert_eq!(parse_query_date("2026-04-02"), expected);
        assert_eq!(parse_query_date("2026-04-02T00:00:00+07:00"), expected);
        // `+` URL-decodes to a space; only the leading date matters.
        assert_eq!(parse_query_date("2026-04-02T00:00:00 07:00"), expected);
        assert_eq!(parse_query_date("nope"), None);
    }
}
