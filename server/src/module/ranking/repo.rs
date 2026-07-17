use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};

use super::language::Language;
use super::models::{
    build_doctor_name, build_specialty_desc, map_language_code, ChannelInfo, DoctorProfile,
    DoctorRow, LocalizedName, PageToken, RankedDoctorInfo, SpecialtyInfo, WorkplaceInfo,
};

const DEFAULT_DURATION_MINUTES: i32 = 15;
const DEFAULT_CHANNEL_TYPES: &[&str] = &["voice", "chat", "video"];
const FEE_CURRENCY: &str = "THB";

#[async_trait]
pub trait RankingRepoTrait: Send + Sync {
    async fn list_doctors(
        &self,
        mode: ListMode,
        page_token: Option<&PageToken>,
        size: u32,
        lang: &Language,
    ) -> AppResult<Vec<DoctorProfile>>;

    async fn get_doctor(
        &self,
        doctor_id: Uuid,
        lang: &Language,
    ) -> AppResult<Option<DoctorProfile>>;

    async fn get_all_ranked_doctors(&self) -> AppResult<Vec<RankedDoctorInfo>>;

    async fn count_doctors(&self, mode: ListMode) -> AppResult<i64>;

    async fn get_doctor_score(&self, doctor_id: Uuid) -> AppResult<Option<i32>>;
}

#[derive(Debug, Clone, Copy)]
pub enum ListMode {
    Instant,
    Scheduled,
}

pub struct RankingRepo {
    pool: PgPool,
    platform_fee_multiplier: f64,
}

impl RankingRepo {
    pub fn new(pool: PgPool, platform_fee_multiplier: f64) -> Self {
        Self {
            pool,
            platform_fee_multiplier,
        }
    }
}

#[async_trait]
impl RankingRepoTrait for RankingRepo {
    async fn list_doctors(
        &self,
        mode: ListMode,
        page_token: Option<&PageToken>,
        size: u32,
        lang: &Language,
    ) -> AppResult<Vec<DoctorProfile>> {
        let mode_filter = match mode {
            ListMode::Instant => {
                "EXISTS (
                    SELECT 1
                    FROM consultation_instant ci
                    WHERE ci.doctor_id = p.doctor_account_id
                      AND ci.is_available = true
                )"
            }
            ListMode::Scheduled => {
                "EXISTS (
                    SELECT 1
                    FROM consultation_schedule cs
                    WHERE cs.doctor_id = p.doctor_account_id
                      AND cs.is_available = true
                )"
            }
        };

        let (pagination_clause, bind_offset) = if page_token.is_some() {
            ("AND (COALESCE(ds.score, 0), p.doctor_id) < ($1, $2)", 2)
        } else {
            ("", 0)
        };

        let query = format!(
            r#"
            WITH ranked_page AS (
                SELECT
                    p.doctor_id,
                    COALESCE(ds.score, 0) AS score
                FROM doctor_profile p
                LEFT JOIN doctor_score ds ON p.doctor_id = ds.doctor_id
                WHERE {mode_filter}
                  AND p.is_active = true
                  {pagination_clause}
                ORDER BY COALESCE(ds.score, 0) DESC, p.doctor_id DESC
                LIMIT ${limit_param}
            )
            SELECT
                p.doctor_id,
                p.first_name AS firstname,
                p.last_name AS lastname,
                p.profile_image_url,
                COALESCE(c.supported_languages, '{{th,en}}'::language_code_enum[])::text[] AS supported_languages,
                p.department_id,
                d.name AS department_name,
                d.counseling_areas AS department_counseling_areas,
                rp.score,
                dr.rating::float8 as rating,
                dc.case_amount,
                c.doctor_fee_amount::float8 as fee_amount,
                c.duration_minutes as default_duration,
                COALESCE(c.channel_types, '{{voice,chat,video}}'::channel_type_enum[])::text[] AS channel_types,
                p.specialty,
                p.work_place,
                p.additional_workplace
            FROM ranked_page rp
            JOIN doctor_profile p ON p.doctor_id = rp.doctor_id
            LEFT JOIN doctor_consultation_config c ON p.doctor_id = c.doctor_id
            LEFT JOIN department d ON p.department_id = d.department_id
            LEFT JOIN doctor_rating dr ON p.doctor_id = dr.doctor_id
            LEFT JOIN doctor_case dc ON p.doctor_id = dc.doctor_id
            ORDER BY rp.score DESC, p.doctor_id DESC
            "#,
            limit_param = bind_offset + 1,
        );

        let rows = if let Some(token) = page_token {
            sqlx::query_as::<_, DoctorRow>(&query)
                .bind(token.s)
                .bind(token.u)
                .bind(size as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?
        } else {
            sqlx::query_as::<_, DoctorRow>(&query)
                .bind(size as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?
        };

        self.build_profiles_batch(&rows, lang).await
    }

    async fn get_doctor(
        &self,
        doctor_id: Uuid,
        lang: &Language,
    ) -> AppResult<Option<DoctorProfile>> {
        let row = sqlx::query_as::<_, DoctorRow>(
            r#"
            SELECT
                p.doctor_id,
                p.first_name AS firstname,
                p.last_name AS lastname,
                p.profile_image_url,
                COALESCE(c.supported_languages, '{th,en}'::language_code_enum[])::text[] AS supported_languages,
                p.department_id,
                d.name AS department_name,
                d.counseling_areas AS department_counseling_areas,
                COALESCE(ds.score, 0) as score,
                dr.rating::float8 as rating,
                dc.case_amount,
                c.doctor_fee_amount::float8 as fee_amount,
                c.duration_minutes as default_duration,
                COALESCE(c.channel_types, '{voice,chat,video}'::channel_type_enum[])::text[] AS channel_types,
                p.specialty,
                p.work_place,
                p.additional_workplace
            FROM doctor_profile p
            LEFT JOIN doctor_score ds ON p.doctor_id = ds.doctor_id
            LEFT JOIN doctor_consultation_config c ON p.doctor_id = c.doctor_id
            LEFT JOIN department d ON p.department_id = d.department_id
            LEFT JOIN doctor_rating dr ON p.doctor_id = dr.doctor_id
            LEFT JOIN doctor_case dc ON p.doctor_id = dc.doctor_id
            WHERE p.doctor_id = $1
            "#,
        )
        .bind(doctor_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        match row {
            Some(ref r) => {
                let profile = self.build_profile(r, lang).await?;
                Ok(Some(profile))
            }
            None => Ok(None),
        }
    }

    async fn get_all_ranked_doctors(&self) -> AppResult<Vec<RankedDoctorInfo>> {
        let rows = sqlx::query_as::<_, RankedDoctorInfo>(
            r#"
            SELECT
                p.doctor_id,
                COALESCE(ds.score, 0) AS score,
                EXISTS (
                    SELECT 1
                    FROM consultation_instant ci
                    WHERE ci.doctor_id = p.doctor_account_id
                      AND ci.is_available = true
                ) AS instant_mode_enabled,
                EXISTS (
                    SELECT 1
                    FROM consultation_schedule cs
                    WHERE cs.doctor_id = p.doctor_account_id
                      AND cs.is_available = true
                ) AS schedule_mode_enabled
            FROM doctor_profile p
            LEFT JOIN doctor_score ds ON p.doctor_id = ds.doctor_id
            WHERE p.is_active = true
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(rows)
    }

    async fn count_doctors(&self, mode: ListMode) -> AppResult<i64> {
        let mode_filter = match mode {
            ListMode::Instant => {
                "EXISTS (
                    SELECT 1
                    FROM consultation_instant ci
                    WHERE ci.doctor_id = p.doctor_account_id
                      AND ci.is_available = true
                )"
            }
            ListMode::Scheduled => {
                "EXISTS (
                    SELECT 1
                    FROM consultation_schedule cs
                    WHERE cs.doctor_id = p.doctor_account_id
                      AND cs.is_available = true
                )"
            }
        };

        let query = format!(
            r#"
            SELECT COUNT(*) as count
            FROM doctor_profile p
            WHERE {mode_filter}
              AND p.is_active = true
            "#
        );

        let count: (i64,) = sqlx::query_as(&query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(count.0)
    }

    async fn get_doctor_score(&self, doctor_id: Uuid) -> AppResult<Option<i32>> {
        let row: Option<(i32,)> = sqlx::query_as(
            r#"
            SELECT COALESCE(ds.score, 0) AS score
            FROM doctor_profile p
            LEFT JOIN doctor_score ds ON p.doctor_id = ds.doctor_id
            WHERE p.doctor_id = $1
            "#,
        )
        .bind(doctor_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(row.map(|r| r.0))
    }
}

impl RankingRepo {
    /// Build a single doctor profile (used by get_doctor for single-item lookups).
    async fn build_profile(&self, row: &DoctorRow, lang: &Language) -> AppResult<DoctorProfile> {
        let profiles = self
            .build_profiles_batch(std::slice::from_ref(row), lang)
            .await?;
        Ok(profiles
            .into_iter()
            .next()
            .expect("single row must produce single profile"))
    }

    /// Build profiles for multiple doctors using batch queries (avoids N+1).
    async fn build_profiles_batch(
        &self,
        rows: &[DoctorRow],
        lang: &Language,
    ) -> AppResult<Vec<DoctorProfile>> {
        if rows.is_empty() {
            return Ok(vec![]);
        }

        let profiles = rows
            .iter()
            .map(|row| {
                let platform_price = row.fee_amount.unwrap_or(0.0) * self.platform_fee_multiplier;
                let duration = row.default_duration.unwrap_or(DEFAULT_DURATION_MINUTES);

                DoctorProfile {
                    usid: row.doctor_id.to_string(),
                    name: build_doctor_name(&row.firstname, &row.lastname, lang),
                    profile_image: row.profile_image_url.clone(),
                    specialties: build_specialties(
                        &row.specialty,
                        row.department_id,
                        row.department_name.as_ref(),
                        lang,
                    ),
                    channels: build_channels(&row.channel_types, duration, platform_price),
                    work_place: build_workplaces(&row.work_place, lang),
                    counseling_areas: build_counseling_areas(
                        row.department_counseling_areas.as_ref(),
                        lang,
                    ),
                    work_experience: build_work_experience(
                        &row.work_place,
                        &row.additional_workplace,
                        lang,
                    ),
                    specialty_desc: build_specialty_desc(&row.specialty, lang),
                    consultation_case: row.case_amount.unwrap_or(0),
                    rating: row.rating.unwrap_or(0.0),
                    available_language: row
                        .supported_languages
                        .iter()
                        .map(|l| map_language_code(l))
                        .collect(),
                    consultation_fee: platform_price,
                    consultation_duration: duration,
                    department_id: row.department_id,
                    associate_privileges: vec![],
                    score: row.score,
                    ranked: 0,
                    i_ranked: 0,
                }
            })
            .collect();

        Ok(profiles)
    }
}

fn capitalize_channel_type(channel_type: &str) -> String {
    match channel_type {
        "chat" => "Chat".to_string(),
        "video" => "Video".to_string(),
        "voice" => "Voice".to_string(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        }
    }
}

fn build_channels(channel_types: &[String], duration: i32, price: f64) -> Vec<ChannelInfo> {
    let channel_types: Vec<&str> = if channel_types.is_empty() {
        DEFAULT_CHANNEL_TYPES.to_vec()
    } else {
        channel_types.iter().map(String::as_str).collect()
    };

    channel_types
        .into_iter()
        .map(|channel_type| ChannelInfo {
            channel_type: capitalize_channel_type(channel_type),
            duration,
            price,
            currency: FEE_CURRENCY.to_string(),
        })
        .collect()
}

fn build_specialties(
    specialty: &serde_json::Value,
    department_id: Option<i32>,
    department_name: Option<&serde_json::Value>,
    lang: &Language,
) -> Vec<SpecialtyInfo> {
    if let Some(id) = department_id {
        let name = department_name
            .and_then(|name_json| localized_name(Some(name_json), lang))
            .filter(|name| !name.is_empty())
            .or_else(|| specialty_display_name(specialty, lang))
            .unwrap_or_else(|| "General".to_string());

        return vec![SpecialtyInfo {
            id,
            name,
            lang_code: lang.lang_code().to_string(),
        }];
    }

    specialty_items(specialty)
        .into_iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_i64()? as i32;
            let name = localized_name(item.get("name"), lang).unwrap_or_else(|| {
                localized_name(item.get("description"), lang)
                    .unwrap_or_else(|| "General".to_string())
            });
            Some(SpecialtyInfo {
                id,
                name,
                lang_code: lang.lang_code().to_string(),
            })
        })
        .collect()
}

fn specialty_display_name(specialty: &serde_json::Value, lang: &Language) -> Option<String> {
    specialty_items(specialty).into_iter().find_map(|item| {
        localized_name(item.get("name"), lang)
            .or_else(|| localized_name(item.get("description"), lang))
            .filter(|name| !name.is_empty())
    })
}

fn specialty_items(specialty: &serde_json::Value) -> Vec<&serde_json::Value> {
    if let Some(items) = specialty.as_array() {
        return items.iter().collect();
    }

    if specialty.as_object().is_some() {
        return vec![specialty];
    }

    vec![]
}

fn build_workplaces(work_place: &serde_json::Value, lang: &Language) -> Vec<WorkplaceInfo> {
    work_place
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let id = item.get("id")?.as_i64()? as i32;
                    let name = localized_name(item.get("name"), lang)
                        .or_else(|| localized_name(item.get("description"), lang))
                        .unwrap_or_default();
                    Some(WorkplaceInfo {
                        id,
                        name,
                        lang_code: lang.lang_code().to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn build_work_experience(
    work_place: &serde_json::Value,
    additional_workplace: &serde_json::Value,
    lang: &Language,
) -> Vec<LocalizedName> {
    work_place
        .as_array()
        .into_iter()
        .chain(additional_workplace.as_array())
        .flat_map(|items| items.iter())
        .filter_map(|item| {
            let workplace_name = localized_name(item.get("name"), lang)
                .or_else(|| localized_name(item.get("description"), lang))
                .filter(|name| !name.trim().is_empty())?;

            Some(LocalizedName {
                lang_code: lang.lang_code().to_string(),
                name: workplace_name,
            })
        })
        .collect()
}

fn build_counseling_areas(
    counseling_areas: Option<&serde_json::Value>,
    lang: &Language,
) -> Vec<LocalizedName> {
    counseling_areas
        .and_then(|areas| localized_name(Some(areas), lang))
        .filter(|name| !name.is_empty())
        .map(|name| {
            vec![LocalizedName {
                lang_code: lang.lang_code().to_string(),
                name,
            }]
        })
        .unwrap_or_default()
}

fn localized_name(value: Option<&serde_json::Value>, lang: &Language) -> Option<String> {
    let value = value?;
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    value
        .get(lang.json_key())
        .or_else(|| value.get("th"))
        .or_else(|| value.get("en"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
}
