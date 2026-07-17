use jiff::{civil::Date, tz::TimeZone, ToSpan};
use tdh_protocol::timeslot::timeslot::DoctorTimeslot;

use crate::doctor_actor::model::{
    AdHocSchedule, DoctorReservation, DoctorScheduleConfig, TimeRange,
};

pub fn generate_timeslots(
    schedule_conf: &DoctorScheduleConfig,
    from_date: Date,
    to_date: Date,
    reservations: &[DoctorReservation],
) -> Result<Vec<DoctorTimeslot>, anyhow::Error> {
    // Index routine by day-of-week (0=Sun..6=Sat) into a fixed array to avoid HashMap overhead
    let mut dow_index: [Option<&[TimeRange]>; 7] = [None; 7];
    for schedule in &schedule_conf.routine {
        let i = schedule.day_of_week as usize;
        if i < 7 {
            dow_index[i] = Some(&schedule.times);
        }
    }

    // Pre-sort ad-hoc by date for O(log n) binary search instead of O(n) linear scan per day
    let mut adhoc_sorted: Vec<&AdHocSchedule> = schedule_conf.ad_hoc.iter().collect();
    adhoc_sorted.sort_unstable_by_key(|a| a.date);

    // Pre-convert reservations to plain i64 seconds pairs — avoids struct field indirection
    // and repeated Timestamp method calls inside the hot loop
    let res_secs: Vec<(i64, i64)> = reservations
        .iter()
        .map(|r| (r.reserved_from.as_second(), r.reserved_until.as_second()))
        .collect();

    let duration = schedule_conf.slot_duration;
    let num_days = from_date.until(to_date)?.get_days() + 1;
    let mut timeslots = Vec::with_capacity(num_days as usize * 4);
    let mut slot_id = 1i64;

    for current_date in from_date.series(1.day()).take_while(|&d| d <= to_date) {
        // Compute midnight epoch seconds once per day — all slot timestamps become
        // cheap integer additions: slot_start = midnight_secs + s * 60
        let midnight_secs = current_date
            .at(0, 0, 0, 0)
            .to_zoned(TimeZone::UTC)?
            .timestamp()
            .as_second();

        // Binary search ad-hoc; fall back to routine dow index
        let time_ranges: &[TimeRange] =
            match adhoc_sorted.binary_search_by_key(&current_date, |a| a.date) {
                Ok(idx) => &adhoc_sorted[idx].times,
                Err(_) => {
                    let dow = current_date.weekday().to_sunday_zero_offset() as usize;
                    match dow_index[dow] {
                        Some(ranges) => ranges,
                        None => continue,
                    }
                }
            };

        for time_range in time_ranges {
            let start_min =
                time_range.start_time.hour() as i32 * 60 + time_range.start_time.minute() as i32;
            let end_min =
                time_range.end_time.hour() as i32 * 60 + time_range.end_time.minute() as i32;

            let slot_count = (end_min - start_min) / duration;
            timeslots.reserve(slot_count as usize);

            // Carry start_time forward to avoid recomputing h/m for each slot start:
            // the current slot's start_time == the previous slot's end_time
            let mut start_time =
                jiff::civil::Time::new((start_min / 60) as i8, (start_min % 60) as i8, 0, 0)?;

            for i in 0..slot_count {
                let s = start_min + i * duration;
                let e = s + duration;

                let end_time = jiff::civil::Time::new((e / 60) as i8, (e % 60) as i8, 0, 0)?;

                // Slot timestamps as plain integer arithmetic — no jiff calls in hot path
                let slot_start_s = midnight_secs + s as i64 * 60;
                let slot_end_s = midnight_secs + e as i64 * 60;

                let overlaps = res_secs
                    .iter()
                    .any(|&(from, until)| slot_start_s < until && slot_end_s > from);

                if !overlaps {
                    timeslots.push(DoctorTimeslot {
                        slot_id,
                        slot_date: current_date,
                        start_time,
                        end_time,
                    });
                    slot_id += 1;
                }

                // Carry end → next start (avoids one Time::new call per slot)
                start_time = end_time;
            }
        }
    }

    Ok(timeslots)
}

/// Generate timeslots for a full day (00:00–24:00) with the given slot duration in minutes,
/// excluding slots that overlap with any reserved `(start_time, end_time)` pair.
///
/// All times are treated in Bangkok timezone (UTC+7). The `reserved` pairs are
/// `(start_time, end_time)` as `jiff::civil::Time` values from the `doctor_reservations` table.
pub fn generate_full_day_timeslots(
    date: Date,
    slot_duration_minutes: i32,
    reserved: &[(jiff::civil::Time, jiff::civil::Time)],
) -> Vec<DoctorTimeslot> {
    let total_minutes = 24 * 60;
    let slot_count = total_minutes / slot_duration_minutes;

    // Pre-convert reserved times to minute offsets for fast overlap checks
    let res_mins: Vec<(i32, i32)> = reserved
        .iter()
        .map(|(s, e)| {
            let s_min = s.hour() as i32 * 60 + s.minute() as i32;
            let e_min = e.hour() as i32 * 60 + e.minute() as i32;
            // Handle midnight crossing: if end <= start, treat end as next day
            let e_min = if e_min <= s_min {
                e_min + 24 * 60
            } else {
                e_min
            };
            (s_min, e_min)
        })
        .collect();

    let mut timeslots = Vec::with_capacity(slot_count as usize);
    let mut slot_id = 1i64;

    for i in 0..slot_count {
        let s = i * slot_duration_minutes;
        let e = s + slot_duration_minutes;

        let overlaps = res_mins.iter().any(|&(rs, re)| s < re && e > rs);

        if !overlaps {
            let start_time = jiff::civil::Time::new((s / 60) as i8, (s % 60) as i8, 0, 0).unwrap();
            let end_time =
                jiff::civil::Time::new(((e / 60) % 24) as i8, (e % 60) as i8, 0, 0).unwrap();

            timeslots.push(DoctorTimeslot {
                slot_id,
                slot_date: date,
                start_time,
                end_time,
            });
            slot_id += 1;
        }
    }

    timeslots
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use jiff::{civil::Time, Timestamp};

    use crate::doctor_actor::model::{
        AdHocSchedule, DoctorReservation, DoctorScheduleConfig, RoutineSchedule, TimeRange,
    };

    use super::*;

    fn monday_schedule(start: &str, end: &str, slot_duration: i32) -> DoctorScheduleConfig {
        DoctorScheduleConfig {
            routine: vec![RoutineSchedule {
                day_of_week: 1, // Monday
                times: vec![TimeRange {
                    start_time: Time::from_str(start).unwrap(),
                    end_time: Time::from_str(end).unwrap(),
                }],
            }],
            ad_hoc: Vec::new(),
            slot_duration,
        }
    }

    fn reservation(date: Date, start: &str, end: &str) -> DoctorReservation {
        DoctorReservation {
            reservation_id: 1,
            reserved_from: date_time_utc(date, start),
            reserved_until: date_time_utc(date, end),
        }
    }

    fn date_time_utc(date: Date, time_str: &str) -> Timestamp {
        let t = Time::from_str(time_str).unwrap();
        date.at(t.hour(), t.minute(), 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
            .timestamp()
    }

    // ── 1. Empty result when schedule config has no routine ──────────────────
    #[test]
    fn test_empty_result_on_empty_config() {
        let from_date = Date::new(2024, 1, 1).unwrap();
        let to_date = Date::new(2024, 1, 7).unwrap();

        let schedule_config = DoctorScheduleConfig {
            routine: Vec::new(),
            ad_hoc: Vec::new(),
            slot_duration: 30,
        };

        let result = generate_timeslots(&schedule_config, from_date, to_date, &[]).unwrap();
        assert!(result.is_empty(), "expected no slots when routine is empty");
    }

    // ── 2. Empty result when reservations cover all available slots ──────────
    #[test]
    fn test_empty_result_when_reservations_overlap_all_slots() {
        // 2024-01-01 is Monday
        let date = Date::new(2024, 1, 1).unwrap();
        let schedule_config = monday_schedule("09:00", "10:00", 30);

        // One reservation covers the entire working window
        let reservations = vec![reservation(date, "09:00", "10:00")];

        let result = generate_timeslots(&schedule_config, date, date, &reservations).unwrap();
        assert!(
            result.is_empty(),
            "expected no slots when all are reserved, got: {result:?}"
        );
    }

    // ── 3. Some slots available when only part of the window is reserved ─────
    #[test]
    fn test_partial_slots_available_with_reservation() {
        // 2024-01-01 is Monday, schedule 09:00-10:00 → slots 09:00-09:30 and 09:30-10:00
        let date = Date::new(2024, 1, 1).unwrap();
        let schedule_config = monday_schedule("09:00", "10:00", 30);

        // Reserve only the first slot
        let reservations = vec![reservation(date, "09:00", "09:30")];

        let result = generate_timeslots(&schedule_config, date, date, &reservations).unwrap();

        assert_eq!(result.len(), 1, "expected 1 available slot");
        assert_eq!(result[0].start_time, Time::from_str("09:30").unwrap());
        assert_eq!(result[0].end_time, Time::from_str("10:00").unwrap());
    }

    // ── 4. Ad-hoc overrides routine; reserved slot within ad-hoc is removed ──
    #[test]
    fn test_partial_slots_with_adhoc_and_reservation() {
        // 2024-01-01 is Monday; routine says 09:00-10:00
        // ad-hoc overrides that day with 13:00-14:00 (two 30-min slots)
        let date = Date::new(2024, 1, 1).unwrap();
        let schedule_config = DoctorScheduleConfig {
            routine: vec![RoutineSchedule {
                day_of_week: 1,
                times: vec![TimeRange {
                    start_time: Time::from_str("09:00").unwrap(),
                    end_time: Time::from_str("10:00").unwrap(),
                }],
            }],
            ad_hoc: vec![AdHocSchedule {
                date,
                times: vec![TimeRange {
                    start_time: Time::from_str("13:00").unwrap(),
                    end_time: Time::from_str("14:00").unwrap(),
                }],
            }],
            slot_duration: 30,
        };

        // Reserve the first ad-hoc slot 13:00-13:30
        let reservations = vec![reservation(date, "13:00", "13:30")];

        let result = generate_timeslots(&schedule_config, date, date, &reservations).unwrap();

        assert_eq!(result.len(), 1, "expected 1 ad-hoc slot remaining");
        assert_eq!(result[0].start_time, Time::from_str("13:30").unwrap());
        assert_eq!(result[0].end_time, Time::from_str("14:00").unwrap());
    }

    // ── 5. Instant reserve at xx:15 overlaps both xx:00 and xx:30 slots ──────
    //
    // Scenario: slots are 09:00-09:30 and 09:30-10:00.
    // A reservation from 09:15 to 09:45 overlaps BOTH slots:
    //   - 09:00-09:30: slot_start(09:00) < res_end(09:45) AND slot_end(09:30) > res_start(09:15) ✓
    //   - 09:30-10:00: slot_start(09:30) < res_end(09:45) AND slot_end(10:00) > res_start(09:15) ✓
    #[test]
    fn test_mid_slot_reservation_removes_both_overlapping_slots() {
        let date = Date::new(2024, 1, 1).unwrap(); // Monday
        let schedule_config = monday_schedule("09:00", "10:00", 30);

        // Instant reserve at 09:15 lasting 30 min — straddles the slot boundary
        let reservations = vec![reservation(date, "09:15", "09:45")];

        let result = generate_timeslots(&schedule_config, date, date, &reservations).unwrap();

        assert!(
            result.is_empty(),
            "expected both slots removed by mid-slot reservation at 09:15, got: {result:?}"
        );
    }

    #[test]
    fn test_full_day_no_reservations_30min() {
        let date = Date::new(2024, 6, 15).unwrap();
        let result = super::generate_full_day_timeslots(date, 30, &[]);
        // 24 hours * 2 slots/hour = 48 slots
        assert_eq!(result.len(), 48);
        assert_eq!(result[0].start_time, Time::new(0, 0, 0, 0).unwrap());
        assert_eq!(result[0].end_time, Time::new(0, 30, 0, 0).unwrap());
        assert_eq!(result[47].start_time, Time::new(23, 30, 0, 0).unwrap());
        assert_eq!(result[47].end_time, Time::new(0, 0, 0, 0).unwrap());
    }

    #[test]
    fn test_full_day_with_reservation_excludes_overlapping() {
        let date = Date::new(2024, 6, 15).unwrap();
        let reserved = vec![
            (
                Time::new(9, 0, 0, 0).unwrap(),
                Time::new(9, 30, 0, 0).unwrap(),
            ),
            (
                Time::new(14, 0, 0, 0).unwrap(),
                Time::new(14, 30, 0, 0).unwrap(),
            ),
        ];
        let result = super::generate_full_day_timeslots(date, 30, &reserved);
        // 48 - 2 = 46
        assert_eq!(result.len(), 46);
        // Verify the reserved slots are not present
        assert!(!result
            .iter()
            .any(|s| s.start_time == Time::new(9, 0, 0, 0).unwrap()));
        assert!(!result
            .iter()
            .any(|s| s.start_time == Time::new(14, 0, 0, 0).unwrap()));
    }

    #[test]
    fn test_full_day_mid_slot_reservation_removes_both() {
        let date = Date::new(2024, 6, 15).unwrap();
        // Reservation from 09:15-09:45 overlaps both 09:00-09:30 and 09:30-10:00
        let reserved = vec![(
            Time::new(9, 15, 0, 0).unwrap(),
            Time::new(9, 45, 0, 0).unwrap(),
        )];
        let result = super::generate_full_day_timeslots(date, 30, &reserved);
        assert_eq!(result.len(), 46); // 48 - 2
        assert!(!result
            .iter()
            .any(|s| s.start_time == Time::new(9, 0, 0, 0).unwrap()));
        assert!(!result
            .iter()
            .any(|s| s.start_time == Time::new(9, 30, 0, 0).unwrap()));
    }

    #[test]
    fn test_full_day_60min_slots() {
        let date = Date::new(2024, 6, 15).unwrap();
        let result = super::generate_full_day_timeslots(date, 60, &[]);
        assert_eq!(result.len(), 24);
    }

    // ── existing baseline test ────────────────────────────────────────────────
    #[test]
    fn test_generate_timeslots_no_reservation() {
        let from_date = Date::new(2024, 1, 1).unwrap(); // Monday
        let to_date = Date::new(2024, 1, 7).unwrap();
        let schedule_config = monday_schedule("09:00", "10:00", 30);

        let result = generate_timeslots(&schedule_config, from_date, to_date, &[]).unwrap();

        let expected: Vec<DoctorTimeslot> = serde_json::from_value(serde_json::json!([
            { "slotId": 1, "slotDate": "2024-01-01", "startTime": "09:00", "endTime": "09:30" },
            { "slotId": 2, "slotDate": "2024-01-01", "startTime": "09:30", "endTime": "10:00" }
        ]))
        .unwrap();

        pretty_assertions::assert_eq!(result, expected);
    }
}
