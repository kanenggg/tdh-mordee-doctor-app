use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::validate::{validate_schedule_config, ScheduleConfigValidationError};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimePeriod {
    pub start_time: i32,
    pub end_time: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DateWithTimePeriods {
    #[schema(value_type = String, format = Date, example = "2026-05-20")]
    pub date: String,
    pub periods: Vec<TimePeriod>,
}

/// Default IANA timezone applied to configs that don't specify one (legacy rows,
/// requests omitting the field). The 90-day window math is anchored to Bangkok
/// regardless; this value is descriptive metadata for downstream consumers.
pub fn default_timezone() -> String {
    "Asia/Bangkok".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleAvailableConfig {
    pub specific_date: Vec<DateWithTimePeriods>,
    #[serde(default = "default_timezone")]
    #[schema(value_type = String, example = "Asia/Bangkok")]
    pub timezone: String,
    #[serde(alias = "dayOfWeek")]
    pub days_of_week: BTreeMap<i32, Vec<TimePeriod>>,
}

impl Default for ScheduleAvailableConfig {
    fn default() -> Self {
        Self {
            specific_date: Vec::new(),
            timezone: default_timezone(),
            days_of_week: BTreeMap::new(),
        }
    }
}

impl ScheduleAvailableConfig {
    pub fn validate(&self) -> Result<(), ScheduleConfigValidationError> {
        validate_schedule_config(self)
    }
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
#[serde(tag = "__type")]
pub enum UpdateScheduleConfigResponse {
    Success,
    #[serde(rename = "Failure.ConflictTimeOverlap")]
    ConflictTimeOverlap {
        days: Vec<i32>,
    },
}
