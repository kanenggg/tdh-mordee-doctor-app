use crate::core::error::AppResult;
use crate::model::onboarding::OnBoarding;
use crate::repo::firestore_repo::{FirestoreRepo, FirestoreRepoTrait, QueryFilter, QueryOp};

#[derive(Clone)]
pub struct BackofficeRepo {
    firestore: FirestoreRepo,
    collection: String,
}

impl BackofficeRepo {
    pub fn new(firestore: FirestoreRepo, collection: String) -> Self {
        Self {
            firestore,
            collection,
        }
    }

    pub async fn list_doctors(
        &self,
        status_filter: Option<&str>,
        page: u32,
        limit: u32,
    ) -> AppResult<Vec<OnBoarding>> {
        let mut filters = Vec::new();
        if let Some(status) = status_filter {
            filters.push(QueryFilter {
                field: "status.__type".to_string(),
                op: QueryOp::Eq,
                value: serde_json::json!(status),
            });
        }
        let offset = page.saturating_sub(1) * limit;
        self.firestore
            .query_collection::<OnBoarding>(
                &self.collection,
                filters,
                None,
                Some(limit),
                Some(offset),
            )
            .await
    }

    pub async fn get_doctor(&self, doctor_id: i32) -> AppResult<Option<OnBoarding>> {
        self.firestore
            .get_doc(&self.collection, &doctor_id.to_string())
            .await
    }

    pub async fn create_doctor(&self, doctor_id: i32, info: &OnBoarding) -> AppResult<()> {
        self.firestore
            .set_doc(&self.collection, &doctor_id.to_string(), info)
            .await
    }

    pub async fn update_doctor(&self, doctor_id: i32, info: &OnBoarding) -> AppResult<()> {
        self.firestore
            .set_doc(&self.collection, &doctor_id.to_string(), info)
            .await
    }
}
