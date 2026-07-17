use async_trait::async_trait;
use sqlx::{FromRow, PgPool};
use tracing::warn;

use crate::core::error::{AppError, AppResult};
use crate::model::doctor_specialty::Specialty;
use crate::model::localize::Localized;

#[derive(Debug, Clone)]
pub struct DoctorBasicInfo {
    pub first_name: Localized,
    pub last_name: Localized,
    pub specialty: Option<Specialty>,
}

#[async_trait]
pub trait DoctorBasicRepoTrait: Send + Sync {
    async fn get_doctor_basic(&self, doctor_account_id: i32) -> AppResult<Option<DoctorBasicInfo>>;
}

#[derive(Clone)]
pub struct DoctorBasicRepo {
    pool: PgPool,
}

impl DoctorBasicRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

// Columns come back as JSONB; decode them as serde_json::Value first and
// deserialize into the domain types below.
#[derive(FromRow)]
struct DoctorBasicRow {
    first_name: serde_json::Value,
    last_name: serde_json::Value,
    specialty: serde_json::Value,
}

impl From<DoctorBasicRow> for DoctorBasicInfo {
    fn from(row: DoctorBasicRow) -> Self {
        let empty_loc = || Localized {
            th: String::new(),
            en: String::new(),
        };
        let first_name: Localized = serde_json::from_value(row.first_name).unwrap_or_else(|e| {
            warn!(error = %e, "failed to deserialize first_name");
            empty_loc()
        });
        let last_name: Localized = serde_json::from_value(row.last_name).unwrap_or_else(|e| {
            warn!(error = %e, "failed to deserialize last_name");
            empty_loc()
        });
        let specialty: Option<Specialty> =
            serde_json::from_value(row.specialty).unwrap_or_else(|e| {
                warn!(error = %e, "failed to deserialize specialty");
                None
            });
        Self {
            first_name,
            last_name,
            specialty,
        }
    }
}

#[async_trait]
impl DoctorBasicRepoTrait for DoctorBasicRepo {
    async fn get_doctor_basic(&self, doctor_account_id: i32) -> AppResult<Option<DoctorBasicInfo>> {
        let row = sqlx::query_as::<_, DoctorBasicRow>(
            r#"SELECT first_name, last_name, specialty FROM get_doctor_profile($1)"#,
        )
        .bind(doctor_account_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(format!("get_doctor_basic: {}", e)))?
        .map(DoctorBasicInfo::from);
        Ok(row)
    }
}
