use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tdh_protocol::notification::ScheduledNotificationTask;
use tracing::{debug, error, info};

use crate::config::CloudTasksConfig;
use crate::core::error::{AppError, AppResult};
use crate::core::GcpTokenProvider;

const CLOUD_TASKS_MAX_SCHEDULE_SECONDS: i64 = 30 * 24 * 60 * 60;
const MAX_CHAIN_COUNT: u32 = 12;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateTaskRequest {
    task: TaskBody,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskBody {
    http_request: HttpRequestBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    schedule_time: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HttpRequestBody {
    url: String,
    http_method: String,
    headers: std::collections::HashMap<String, String>,
    body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    oidc_token: Option<OidcToken>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OidcToken {
    service_account_email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    audience: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTaskResponse {
    #[serde(default)]
    name: String,
}

#[derive(Clone)]
pub struct CloudTasksService {
    config: Arc<CloudTasksConfig>,
    http_client: reqwest::Client,
    token_provider: Arc<GcpTokenProvider>,
}

impl CloudTasksService {
    pub fn new(config: &CloudTasksConfig, token_provider: Arc<GcpTokenProvider>) -> Self {
        info!(
            project_id = %config.gcp_project_id,
            location = %config.gcp_location,
            queue = %config.queue_name,
            "Cloud Tasks service initialized"
        );

        Self {
            config: Arc::new(config.clone()),
            http_client: reqwest::Client::new(),
            token_provider,
        }
    }

    pub async fn schedule_notification(
        &self,
        task: ScheduledNotificationTask,
        schedule_time: Timestamp,
    ) -> AppResult<String> {
        if task.chain_count >= MAX_CHAIN_COUNT {
            return Err(AppError::InvalidScheduleTime(format!(
                "Maximum chain count of {} exceeded",
                MAX_CHAIN_COUNT
            )));
        }

        let now = Timestamp::now();
        let effective_schedule = self.calculate_effective_schedule(schedule_time, now);
        let needs_chaining = effective_schedule < schedule_time;

        if needs_chaining {
            info!(
                target_time = %schedule_time,
                effective_time = %effective_schedule,
                chain_count = task.chain_count,
                "Scheduling chained Cloud Task (target beyond 30-day limit)"
            );
        } else {
            info!(
                schedule_time = %effective_schedule,
                "Scheduling Cloud Task for direct delivery"
            );
        }

        let task_name = self.create_cloud_task(&task, effective_schedule).await?;

        info!(
            task_name = %task_name,
            "Cloud Task created successfully"
        );

        Ok(task_name)
    }

    pub fn should_send_now(&self, task: &ScheduledNotificationTask) -> bool {
        let now = Timestamp::now();
        let remaining = task.original_schedule_time - now;
        remaining.is_negative() || remaining.is_zero()
    }

    pub async fn reschedule_chain(&self, mut task: ScheduledNotificationTask) -> AppResult<String> {
        task.chain_count += 1;
        let target = task.original_schedule_time;
        self.schedule_notification(task, target).await
    }

    fn calculate_effective_schedule(&self, target: Timestamp, now: Timestamp) -> Timestamp {
        let max = now
            .checked_add(SignedDuration::from_secs(CLOUD_TASKS_MAX_SCHEDULE_SECONDS))
            .expect("valid timestamp");
        target.min(max)
    }

    async fn create_cloud_task(
        &self,
        task: &ScheduledNotificationTask,
        schedule_time: Timestamp,
    ) -> AppResult<String> {
        let access_token = self.get_access_token().await?;

        let queue_path = format!(
            "projects/{}/locations/{}/queues/{}",
            self.config.gcp_project_id, self.config.gcp_location, self.config.queue_name
        );

        let handler_url = format!("{}/tasks/v1/notification", self.config.handler_base_url);

        let body_json = serde_json::to_string(task).map_err(|e| {
            AppError::CloudTasksError(format!("Failed to serialize task payload: {e}"))
        })?;

        let body_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            body_json.as_bytes(),
        );

        let mut headers = std::collections::HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        let oidc_token = self.oidc_token()?;

        let request_body = CreateTaskRequest {
            task: TaskBody {
                http_request: HttpRequestBody {
                    url: handler_url,
                    http_method: "POST".to_string(),
                    headers,
                    body: body_b64,
                    oidc_token,
                },
                schedule_time: Some(schedule_time.to_string()),
            },
        };

        let api_url = if let Some(ref emulator) = self.config.emulator_host {
            if !emulator.is_empty() {
                format!("http://{}/v2/{}/tasks", emulator, queue_path)
            } else {
                format!("https://cloudtasks.googleapis.com/v2/{}/tasks", queue_path)
            }
        } else {
            format!("https://cloudtasks.googleapis.com/v2/{}/tasks", queue_path)
        };

        debug!(url = %api_url, "Creating Cloud Task");

        let response = self
            .http_client
            .post(&api_url)
            .bearer_auth(&access_token)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                AppError::CloudTasksError(format!("HTTP request to Cloud Tasks API failed: {e}"))
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!(
                status = %status,
                body = %error_text,
                "Cloud Tasks API returned error"
            );
            return Err(AppError::CloudTasksError(format!(
                "Cloud Tasks API error ({}): {}",
                status, error_text
            )));
        }

        let resp: CreateTaskResponse = response.json().await.map_err(|e| {
            AppError::CloudTasksError(format!("Failed to parse Cloud Tasks response: {e}"))
        })?;

        Ok(resp.name)
    }

    fn oidc_token(&self) -> AppResult<Option<OidcToken>> {
        if self
            .config
            .emulator_host
            .as_ref()
            .is_some_and(|host| !host.is_empty())
        {
            return Ok(None);
        }

        let service_account_email = self.config.oidc_service_account_email.trim();
        if service_account_email.is_empty() {
            return Err(AppError::CloudTasksError(
                "cloud_tasks.oidc_service_account_email is required when Cloud Tasks emulator is not configured".to_string(),
            ));
        }

        let audience = self
            .config
            .oidc_audience
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                Some(
                    self.config
                        .handler_base_url
                        .trim_end_matches('/')
                        .to_string(),
                )
            });

        Ok(Some(OidcToken {
            service_account_email: service_account_email.to_string(),
            audience,
        }))
    }

    pub async fn cancel_task(&self, task_name: &str) -> AppResult<()> {
        if task_name.is_empty() {
            return Ok(());
        }

        let access_token = self.get_access_token().await?;

        let api_url = if let Some(ref emulator) = self.config.emulator_host {
            if !emulator.is_empty() {
                format!("http://{}/v2/{}", emulator, task_name)
            } else {
                format!("https://cloudtasks.googleapis.com/v2/{}", task_name)
            }
        } else {
            format!("https://cloudtasks.googleapis.com/v2/{}", task_name)
        };

        debug!(url = %api_url, task_name = %task_name, "Deleting Cloud Task");

        let response = self
            .http_client
            .delete(&api_url)
            .bearer_auth(&access_token)
            .send()
            .await
            .map_err(|e| {
                AppError::CloudTasksError(format!("HTTP request to delete Cloud Task failed: {e}"))
            })?;

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            info!(task_name = %task_name, "Cloud Task already deleted or not found");
            return Ok(());
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!(
                status = %status,
                task_name = %task_name,
                body = %error_text,
                "Cloud Tasks DELETE API returned error"
            );
            return Err(AppError::CloudTasksError(format!(
                "Cloud Tasks DELETE error ({}): {}",
                status, error_text
            )));
        }

        info!(task_name = %task_name, "Cloud Task deleted successfully");
        Ok(())
    }

    async fn get_access_token(&self) -> AppResult<String> {
        if let Ok(token) = std::env::var("CLOUD_TASKS_ACCESS_TOKEN") {
            if !token.is_empty() {
                debug!("Using Cloud Tasks access token from CLOUD_TASKS_ACCESS_TOKEN env var");
                return Ok(token);
            }
        }
        self.token_provider.token().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tdh_protocol::notification::{NotificationPayload, NotificationType};

    fn test_token_provider() -> Arc<GcpTokenProvider> {
        unsafe {
            std::env::set_var("CLOUD_TASKS_ACCESS_TOKEN", "test-token");
        }
        Arc::new(GcpTokenProvider::new())
    }

    #[test]
    fn test_calculate_effective_schedule_within_30_days() {
        let config = CloudTasksConfig {
            gcp_project_id: "test".to_string(),
            gcp_location: "us-central1".to_string(),
            queue_name: "test-queue".to_string(),
            emulator_host: None,
            handler_base_url: "http://localhost:8081".to_string(),
            oidc_service_account_email: "tasks@example.iam.gserviceaccount.com".to_string(),
            oidc_audience: None,
        };
        let service = CloudTasksService::new(&config, test_token_provider());

        let now = Timestamp::now();
        let target = now
            .checked_add(SignedDuration::from_secs(15 * 24 * 3600))
            .unwrap();

        let effective = service.calculate_effective_schedule(target, now);
        assert_eq!(effective, target);
    }

    #[test]
    fn test_calculate_effective_schedule_beyond_30_days() {
        let config = CloudTasksConfig {
            gcp_project_id: "test".to_string(),
            gcp_location: "us-central1".to_string(),
            queue_name: "test-queue".to_string(),
            emulator_host: None,
            handler_base_url: "http://localhost:8081".to_string(),
            oidc_service_account_email: "tasks@example.iam.gserviceaccount.com".to_string(),
            oidc_audience: None,
        };
        let service = CloudTasksService::new(&config, test_token_provider());

        let now = Timestamp::now();
        let target = now
            .checked_add(SignedDuration::from_secs(45 * 24 * 3600))
            .unwrap();
        let effective = service.calculate_effective_schedule(target, now);
        let max = now
            .checked_add(SignedDuration::from_secs(CLOUD_TASKS_MAX_SCHEDULE_SECONDS))
            .unwrap();
        assert_eq!(effective, max);
    }

    #[test]
    fn test_should_send_now_when_time_reached() {
        let config = CloudTasksConfig {
            gcp_project_id: "test".to_string(),
            gcp_location: "us-central1".to_string(),
            queue_name: "test-queue".to_string(),
            emulator_host: None,
            handler_base_url: "http://localhost:8081".to_string(),
            oidc_service_account_email: "tasks@example.iam.gserviceaccount.com".to_string(),
            oidc_audience: None,
        };
        let service = CloudTasksService::new(&config, test_token_provider());

        let past_time = Timestamp::now()
            .checked_sub(SignedDuration::from_secs(5 * 60))
            .unwrap();
        let task = ScheduledNotificationTask {
            notification: NotificationPayload {
                notification_type: NotificationType::System,
                doctor_account_ids: Some(vec![1]),
                title: "Test".to_string(),
                body: "Test".to_string(),
                data: None,
                category: None,
                scheduled_at: Some(past_time),
            },
            original_schedule_time: past_time,
            chain_count: 0,
        };
        assert!(service.should_send_now(&task));
    }

    #[test]
    fn test_should_send_now_when_time_not_reached() {
        let config = CloudTasksConfig {
            gcp_project_id: "test".to_string(),
            gcp_location: "us-central1".to_string(),
            queue_name: "test-queue".to_string(),
            emulator_host: None,
            handler_base_url: "http://localhost:8081".to_string(),
            oidc_service_account_email: "tasks@example.iam.gserviceaccount.com".to_string(),
            oidc_audience: None,
        };
        let service = CloudTasksService::new(&config, test_token_provider());

        // let future_time_chrono = Utc::now() + Duration::days(15);
        // let future_time = chrono_to_timestamp(future_time_chrono);
        let future_time = Timestamp::now()
            .checked_add(SignedDuration::from_secs(24 * 3600))
            .unwrap();
        let task = ScheduledNotificationTask {
            notification: NotificationPayload {
                notification_type: NotificationType::System,
                doctor_account_ids: Some(vec![1]),
                title: "Test".to_string(),
                body: "Test".to_string(),
                data: None,
                category: None,
                scheduled_at: Some(future_time),
            },
            original_schedule_time: future_time,
            chain_count: 0,
        };
        assert!(!service.should_send_now(&task));
    }
}
