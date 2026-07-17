use std::sync::Arc;

use super::models::{
    ChannelType, GetDoctorConfigurationResponse, LanguageCode, UpdateConfigurationResponse,
};
use super::repo::DoctorConfigurationRepo;
use crate::core::error::{AppError, AppResult};

pub struct DoctorConfigurationService {
    repo: Arc<dyn DoctorConfigurationRepo>,
}

impl DoctorConfigurationService {
    pub fn new(repo: Arc<dyn DoctorConfigurationRepo>) -> Self {
        Self { repo }
    }

    pub async fn get_configuration(
        &self,
        doctor_account_id: i32,
    ) -> AppResult<GetDoctorConfigurationResponse> {
        let config = self.repo.get_configuration(doctor_account_id).await?;
        Ok(match config {
            Some(c) => GetDoctorConfigurationResponse::Found(c),
            None => GetDoctorConfigurationResponse::NotFound,
        })
    }

    pub async fn update_channels(
        &self,
        doctor_account_id: i32,
        channels: Vec<ChannelType>,
    ) -> AppResult<UpdateConfigurationResponse> {
        if channels.is_empty() {
            return Err(AppError::BadRequest(
                "channel must contain at least one value".to_string(),
            ));
        }
        let updated = self
            .repo
            .update_channels(doctor_account_id, &channels)
            .await?;
        Ok(to_update_response(updated))
    }

    pub async fn update_languages(
        &self,
        doctor_account_id: i32,
        languages: Vec<LanguageCode>,
    ) -> AppResult<UpdateConfigurationResponse> {
        if languages.is_empty() {
            return Err(AppError::BadRequest(
                "language must contain at least one value".to_string(),
            ));
        }
        let updated = self
            .repo
            .update_languages(doctor_account_id, &languages)
            .await?;
        Ok(to_update_response(updated))
    }
}

fn to_update_response(updated: bool) -> UpdateConfigurationResponse {
    if updated {
        UpdateConfigurationResponse::Success
    } else {
        UpdateConfigurationResponse::NotFound
    }
}
