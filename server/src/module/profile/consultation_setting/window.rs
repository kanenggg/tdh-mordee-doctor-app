//! Pure, time-injected helpers for the forward-looking schedule window.
//!
//! The schedule config is forward-looking (for patient booking), not a history
//! record. On read we expose only a 90-day forward window; on write we drop any
//! past-dated overrides so the stored blob equals what a read would return.
//!
//! All functions take `today` explicitly so they are deterministic and unit
//! testable. "today" is computed by the service as the current date in Bangkok.

use jiff::{civil::Date, ToSpan};

use super::model::ScheduleAvailableConfig;

/// How far forward (in days, inclusive) the read window extends from today.
pub const FORWARD_WINDOW_DAYS: i64 = 90;

/// GET filter: keep only `specificDate` entries within `[today, today + 90]`
/// (both bounds inclusive). Entries whose date fails to parse are dropped.
/// `daysOfWeek` and `timezone` are left untouched.
pub fn retain_forward_window(config: &mut ScheduleAvailableConfig, today: Date) {
    let end = today
        .checked_add(FORWARD_WINDOW_DAYS.days())
        .unwrap_or(Date::MAX);

    config.specific_date.retain(|entry| {
        entry
            .date
            .parse::<Date>()
            .is_ok_and(|date| date >= today && date <= end)
    });
}

/// PUT strip: drop `specificDate` entries dated strictly before `today`.
/// A client only ever sees the forward window, so it has no business writing the
/// past; this keeps the stored config aligned with what a read returns.
pub fn drop_past_specific_dates(config: &mut ScheduleAvailableConfig, today: Date) {
    config
        .specific_date
        .retain(|entry| entry.date.parse::<Date>().is_ok_and(|date| date >= today));
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use jiff::civil::date;

    use super::super::model::{DateWithTimePeriods, ScheduleAvailableConfig, TimePeriod};
    use super::*;

    fn entry(d: &str) -> DateWithTimePeriods {
        DateWithTimePeriods {
            date: d.to_string(),
            periods: vec![TimePeriod {
                start_time: 540,
                end_time: 600,
            }],
        }
    }

    fn config(dates: &[&str]) -> ScheduleAvailableConfig {
        ScheduleAvailableConfig {
            specific_date: dates.iter().map(|d| entry(d)).collect(),
            timezone: "Asia/Bangkok".to_string(),
            days_of_week: BTreeMap::new(),
        }
    }

    fn dates(config: &ScheduleAvailableConfig) -> Vec<&str> {
        config
            .specific_date
            .iter()
            .map(|e| e.date.as_str())
            .collect()
    }

    #[test]
    fn retain_window_keeps_today_through_today_plus_90_inclusive() {
        let today = date(2026, 6, 15);
        let mut cfg = config(&[
            "2026-06-14", // yesterday — out (past)
            "2026-06-15", // today — in (lower bound inclusive)
            "2026-06-20", // in
            "2026-09-13", // today + 90 — in (upper bound inclusive)
            "2026-09-14", // today + 91 — out
        ]);

        retain_forward_window(&mut cfg, today);

        assert_eq!(dates(&cfg), vec!["2026-06-15", "2026-06-20", "2026-09-13"]);
    }

    #[test]
    fn retain_window_drops_unparseable_dates() {
        let today = date(2026, 6, 15);
        let mut cfg = config(&["not-a-date", "2026-06-20"]);

        retain_forward_window(&mut cfg, today);

        assert_eq!(dates(&cfg), vec!["2026-06-20"]);
    }

    #[test]
    fn retain_window_leaves_days_of_week_and_timezone() {
        let today = date(2026, 6, 15);
        let mut cfg = config(&["2026-01-01"]);
        cfg.days_of_week.insert(
            1,
            vec![TimePeriod {
                start_time: 540,
                end_time: 720,
            }],
        );

        retain_forward_window(&mut cfg, today);

        assert!(cfg.specific_date.is_empty());
        assert_eq!(cfg.timezone, "Asia/Bangkok");
        assert!(cfg.days_of_week.contains_key(&1));
    }

    #[test]
    fn drop_past_keeps_today_and_future_only() {
        let today = date(2026, 6, 15);
        let mut cfg = config(&["2026-03-01", "2026-06-15", "2026-07-10"]);

        drop_past_specific_dates(&mut cfg, today);

        assert_eq!(dates(&cfg), vec!["2026-06-15", "2026-07-10"]);
    }

    #[test]
    fn drop_past_keeps_far_future_unlike_window() {
        // The write-side strip has no upper bound — only the read window caps at +90.
        let today = date(2026, 6, 15);
        let mut cfg = config(&["2027-01-01"]);

        drop_past_specific_dates(&mut cfg, today);

        assert_eq!(dates(&cfg), vec!["2027-01-01"]);
    }
}
