use crate::config::FirestoreConfig;
use crate::core::error::{AppError, AppResult};
use async_trait::async_trait;
use backoff::future::retry_notify;
use backoff::{Error as BackoffError, ExponentialBackoff};
use firestore::{
    firestore_document_from_map, FirestoreDb, FirestoreDbOptions, FirestoreQueryDirection,
    FirestoreValue,
};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct QueryFilter {
    pub field: String,
    pub op: QueryOp,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Copy)]
pub enum QueryOp {
    Eq,
    NotEq,
    Gt,
    Lt,
    GtEq,
    LtEq,
    In,
}

#[async_trait]
pub trait FirestoreRepoTrait: Send + Sync {
    async fn get_doc<T: DeserializeOwned + Send>(
        &self,
        collection: &str,
        doc_id: &str,
    ) -> AppResult<Option<T>>;
    async fn set_doc<T: Serialize + DeserializeOwned + Send + Sync>(
        &self,
        collection: &str,
        doc_id: &str,
        data: &T,
    ) -> AppResult<()>;
    async fn update_doc(
        &self,
        collection: &str,
        doc_id: &str,
        fields: HashMap<String, serde_json::Value>,
    ) -> AppResult<()>;
    async fn set_subcollection_doc<T: Serialize + DeserializeOwned + Send + Sync>(
        &self,
        parent_collection: &str,
        parent_doc_id: &str,
        subcollection: &str,
        doc_id: &str,
        data: &T,
    ) -> AppResult<()>;
    async fn query_subcollection<T: DeserializeOwned + Send>(
        &self,
        parent_collection: &str,
        parent_doc_id: &str,
        subcollection: &str,
        filters: Vec<QueryFilter>,
        order_by: Option<&str>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> AppResult<Vec<T>>;
    async fn delete_subcollection_doc(
        &self,
        parent_collection: &str,
        parent_doc_id: &str,
        subcollection: &str,
        doc_id: &str,
    ) -> AppResult<()>;
    async fn delete_doc(&self, collection: &str, doc_id: &str) -> AppResult<()>;
    async fn batch_update(
        &self,
        updates: Vec<(String, HashMap<String, serde_json::Value>)>,
    ) -> AppResult<()>;
    async fn batch_write_updates(
        &self,
        updates: Vec<(String, HashMap<String, serde_json::Value>)>,
    ) -> AppResult<()>;
    async fn update_doc_partial(
        &self,
        collection: &str,
        doc_id: &str,
        fields: HashMap<String, serde_json::Value>,
        field_paths: Vec<String>,
    ) -> AppResult<()>;
    async fn batch_write_updates_partial(
        &self,
        updates: Vec<(String, HashMap<String, serde_json::Value>)>,
        field_paths: Vec<String>,
    ) -> AppResult<()>;
    async fn batch_update_subcollection_docs(
        &self,
        parent_collection: &str,
        updates: Vec<(&str, &str, &str, HashMap<String, serde_json::Value>)>, // (parent_doc_id, subcollection, doc_id, fields)
    ) -> AppResult<()>;
    async fn query_collection<T: DeserializeOwned + Send>(
        &self,
        collection: &str,
        filters: Vec<QueryFilter>,
        order_by: Option<&str>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> AppResult<Vec<T>>;
    async fn count_subcollection(
        &self,
        parent_collection: &str,
        parent_doc_id: &str,
        subcollection: &str,
        filters: Vec<QueryFilter>,
    ) -> AppResult<u32>;
    async fn count_collection(&self, collection: &str, filters: Vec<QueryFilter>)
        -> AppResult<u32>;
}

#[derive(Clone)]
pub struct FirestoreRepo {
    pub db: FirestoreDb,
    #[allow(dead_code)]
    pub config: FirestoreConfig,
    retry_config: ExponentialBackoff,
}

impl FirestoreRepo {
    pub async fn new(
        config: &FirestoreConfig,
        retry_config: ExponentialBackoff,
    ) -> AppResult<Self> {
        let options = FirestoreDbOptions::new(config.gcp_project_id.clone())
            .with_database_id(config.database_id.clone());
        let db = FirestoreDb::with_options(options)
            .await
            .map_err(|e| AppError::FirestoreError(e.to_string()))?;
        Ok(Self {
            db,
            config: config.clone(),
            retry_config,
        })
    }
}

fn to_firestore_error(e: impl std::fmt::Display) -> AppError {
    AppError::FirestoreError(e.to_string())
}

fn is_not_found_error(err: &AppError) -> bool {
    match err {
        AppError::FirestoreError(msg) => {
            msg.contains("NotFound")
                || msg.contains("not found")
                || msg.contains("Error code: NotFound")
        }
        _ => false,
    }
}

fn is_already_exists_error(err: &AppError) -> bool {
    match err {
        AppError::FirestoreError(msg) => {
            msg.contains("AlreadyExists")
                || msg.contains("already exists")
                || msg.contains("Error code: AlreadyExists")
        }
        _ => false,
    }
}

fn is_retryable_firestore_error(err: &AppError) -> bool {
    match err {
        AppError::FirestoreError(msg) => {
            msg.contains("UNAVAILABLE")
                || msg.contains("DEADLINE_EXCEEDED")
                || msg.contains("RESOURCE_EXHAUSTED")
                || msg.contains("INTERNAL")
                || msg.contains("503")
                || msg.contains("timed out")
                || msg.contains("timeout")
        }
        _ => false,
    }
}

fn json_to_firestore_value(v: &serde_json::Value) -> FirestoreValue {
    use gcloud_sdk::google::firestore::v1::value;
    use gcloud_sdk::google::firestore::v1::{ArrayValue, MapValue, Value};
    match v {
        serde_json::Value::Null => FirestoreValue::from(Value { value_type: None }),
        serde_json::Value::Bool(b) => (*b).into(),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into()
            } else if let Some(f) = n.as_f64() {
                f.into()
            } else {
                (n.as_u64().unwrap_or(0) as i64).into()
            }
        }
        serde_json::Value::String(s) => s.clone().into(),
        serde_json::Value::Array(arr) => {
            let values: Vec<Value> = arr
                .iter()
                .map(|v| json_to_firestore_value(v).value)
                .collect();
            FirestoreValue::from(Value {
                value_type: Some(value::ValueType::ArrayValue(ArrayValue { values })),
            })
        }
        serde_json::Value::Object(obj) => {
            let fields: HashMap<String, Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_firestore_value(v).value))
                .collect();
            FirestoreValue::from(Value {
                value_type: Some(value::ValueType::MapValue(MapValue { fields })),
            })
        }
    }
}

async fn firestore_retry<F, T, Fut>(
    retry_config: &ExponentialBackoff,
    operation: F,
    context: &str,
) -> AppResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = AppResult<T>>,
{
    let mut op = operation;
    retry_notify(
        retry_config.clone(),
        || {
            let result_fut = op();
            async move {
                result_fut.await.map_err(|e| {
                    if is_retryable_firestore_error(&e) {
                        BackoffError::transient(e)
                    } else {
                        BackoffError::permanent(e)
                    }
                })
            }
        },
        |err, next_wait: Duration| {
            tracing::warn!(
                context,
                error = %err,
                wait_ms = next_wait.as_millis(),
                "Firestore operation failed, retrying"
            );
        },
    )
    .await
}

#[async_trait]
impl FirestoreRepoTrait for FirestoreRepo {
    async fn get_doc<T: DeserializeOwned + Send>(
        &self,
        collection: &str,
        doc_id: &str,
    ) -> AppResult<Option<T>> {
        let db = self.db.clone();
        let collection = collection.to_string();
        let doc_id = doc_id.to_string();
        let context = format!("get_doc(collection={}, doc_id={})", collection, doc_id);

        firestore_retry(
            &self.retry_config,
            move || {
                let db = db.clone();
                let collection = collection.clone();
                let doc_id = doc_id.clone();
                async move {
                    db.fluent()
                        .select()
                        .by_id_in(&collection)
                        .obj()
                        .one(&doc_id)
                        .await
                        .map_err(to_firestore_error)
                }
            },
            &context,
        )
        .await
    }

    async fn set_doc<T: Serialize + DeserializeOwned + Send + Sync>(
        &self,
        collection: &str,
        doc_id: &str,
        data: &T,
    ) -> AppResult<()> {
        let db = self.db.clone();
        let collection = collection.to_string();
        let doc_id = doc_id.to_string();
        let data_owned = serde_json::to_value(data)
            .map_err(|e| AppError::FirestoreError(format!("Failed to serialize data: {}", e)))?;
        let context = format!("set_doc(collection={}, doc_id={})", collection, doc_id);

        firestore_retry(
            &self.retry_config,
            move || {
                let db = db.clone();
                let collection = collection.clone();
                let doc_id = doc_id.clone();
                let data = data_owned.clone();
                async move {
                    let data_val: T = serde_json::from_value(data).map_err(|e| {
                        AppError::FirestoreError(format!("Failed to deserialize data: {}", e))
                    })?;
                    db.fluent()
                        .insert()
                        .into(&collection)
                        .document_id(&doc_id)
                        .object(&data_val)
                        .execute::<()>()
                        .await
                        .map_err(to_firestore_error)
                }
            },
            &context,
        )
        .await
    }

    async fn update_doc(
        &self,
        collection: &str,
        doc_id: &str,
        fields: HashMap<String, serde_json::Value>,
    ) -> AppResult<()> {
        if fields.is_empty() {
            return Ok(());
        }

        let db = self.db.clone();
        let collection = collection.to_string();
        let doc_id = doc_id.to_string();
        let context = format!("update_doc(collection={}, doc_id={})", collection, doc_id);
        let database_path = self.db.get_database_path().to_string();

        firestore_retry(
            &self.retry_config,
            move || {
                let db = db.clone();
                let collection = collection.clone();
                let doc_id = doc_id.clone();
                let fields = fields.clone();
                let database_path = database_path.clone();
                async move {
                    let fv_fields: Vec<(String, FirestoreValue)> = fields
                        .into_iter()
                        .map(|(k, v)| (k, json_to_firestore_value(&v)))
                        .collect();

                    // Check if doc_id contains '/' - this indicates a subcollection path like "parent_doc_id/subcollection/doc_id"
                    if doc_id.contains('/') {
                        // Parse subcollection path: "parent_doc_id/subcollection/doc_id"
                        let parts: Vec<&str> = doc_id.split('/').collect();
                        if parts.len() == 3 {
                            // Use dot notation for subcollections: collection.parent_doc_id.subcollection
                            let collection_path =
                                format!("{}.{}.{}", collection, parts[0], parts[1]);
                            let doc_id_only = parts[2];

                            let doc_path = format!(
                                "{}/documents/{}/{}",
                                database_path, collection_path, doc_id_only
                            );
                            let doc = firestore_document_from_map(&doc_path, fv_fields)
                                .map_err(to_firestore_error)?;
                            db.fluent()
                                .update()
                                .in_col(&collection_path)
                                .document(doc)
                                .execute()
                                .await
                                .map_err(to_firestore_error)?;
                            return Ok(());
                        }
                    }

                    // Standard top-level document update
                    let doc_path = format!("{}/documents/{}/{}", database_path, collection, doc_id);
                    let doc = firestore_document_from_map(&doc_path, fv_fields)
                        .map_err(to_firestore_error)?;
                    db.fluent()
                        .update()
                        .in_col(&collection)
                        .document(doc)
                        .execute()
                        .await
                        .map_err(to_firestore_error)?;
                    Ok(())
                }
            },
            &context,
        )
        .await
    }

    async fn update_doc_partial(
        &self,
        collection: &str,
        doc_id: &str,
        fields: HashMap<String, serde_json::Value>,
        field_paths: Vec<String>,
    ) -> AppResult<()> {
        if fields.is_empty() {
            return Ok(());
        }
        let fv_fields: Vec<(String, FirestoreValue)> = fields
            .into_iter()
            .map(|(k, v)| (k, json_to_firestore_value(&v)))
            .collect();

        let doc_path = format!(
            "{}/documents/{}/{}",
            self.db.get_database_path(),
            collection,
            doc_id
        );
        let doc = firestore_document_from_map(&doc_path, fv_fields).map_err(to_firestore_error)?;
        self.db
            .fluent()
            .update()
            .fields(&field_paths)
            .in_col(collection)
            .document(doc)
            .execute()
            .await
            .map_err(to_firestore_error)?;
        Ok(())
    }

    async fn batch_write_updates_partial(
        &self,
        updates: Vec<(String, HashMap<String, serde_json::Value>)>,
        field_paths: Vec<String>,
    ) -> AppResult<()> {
        if updates.is_empty() {
            return Ok(());
        }

        const BATCH_LIMIT: usize = 500;

        let batch_writer = self
            .db
            .create_simple_batch_writer()
            .await
            .map_err(to_firestore_error)?;

        for chunk in updates.chunks(BATCH_LIMIT) {
            let mut batch = batch_writer.new_batch();

            for (doc_path, fields) in chunk {
                let parts: Vec<&str> = doc_path.split('/').collect();

                if parts.len() == 3 {
                    let parent_path = format!("{}/{}", self.db.get_documents_path(), parts[0]);

                    batch
                        .update_object_at(
                            &parent_path,
                            parts[1],
                            parts[2],
                            &fields,
                            Some(field_paths.clone()),
                            None,
                            vec![],
                        )
                        .map_err(to_firestore_error)?;
                } else if parts.len() == 2 {
                    batch
                        .update_object(
                            parts[0],
                            parts[1],
                            &fields,
                            Some(field_paths.clone()),
                            None,
                            vec![],
                        )
                        .map_err(to_firestore_error)?;
                } else {
                    return Err(AppError::FirestoreError(format!(
                        "Invalid doc path format: expected 'collection/doc_id' or 'parent/subcol/doc', got '{}'",
                        doc_path
                    )));
                }
            }

            batch.write().await.map_err(to_firestore_error)?;
        }

        Ok(())
    }

    async fn batch_update(
        &self,
        updates: Vec<(String, HashMap<String, serde_json::Value>)>,
    ) -> AppResult<()> {
        for (doc_path, fields) in updates {
            let parts: Vec<&str> = doc_path.splitn(2, '/').collect();
            if parts.len() != 2 {
                return Err(AppError::FirestoreError(format!(
                    "Invalid doc path format: expected 'collection/doc_id', got '{}'",
                    doc_path
                )));
            }
            let (collection, doc_id) = (parts[0], parts[1]);
            self.update_doc(collection, doc_id, fields).await?;
        }
        Ok(())
    }

    async fn batch_write_updates(
        &self,
        updates: Vec<(String, HashMap<String, serde_json::Value>)>,
    ) -> AppResult<()> {
        if updates.is_empty() {
            return Ok(());
        }

        let db = self.db.clone();
        let updates_clone = updates.clone();
        let context = format!("batch_write_updates(count={})", updates.len());

        firestore_retry(&self.retry_config, move || {
            let db = db.clone();
            let updates = updates_clone.clone();
            async move {
                // Firestore batch limit is 500 operations
                const BATCH_LIMIT: usize = 500;

                let batch_writer = db
                    .create_simple_batch_writer()
                    .await
                    .map_err(to_firestore_error)?;

                // Split into batches if needed
                for chunk in updates.chunks(BATCH_LIMIT) {
                    let mut batch = batch_writer.new_batch();

                    for (doc_path, fields) in chunk {
                        // Parse doc_path: "parent_doc_id/subcollection/doc_id" for subcollections
                        // or just "collection/doc_id" for top-level documents
                        let parts: Vec<&str> = doc_path.split('/').collect();

                        if parts.len() == 3 {
                            // Subcollection path: parent_doc_id/subcollection/doc_id
                            // Need to construct parent path manually for subcollections
                            // Parent path format: projects/{project}/databases/{database}/documents/{parent_collection}/{parent_doc_id}
                            let parent_path = format!("{}/{}", db.get_documents_path(), parts[0]);

                            batch
                                .update_object_at(
                                    &parent_path,
                                    parts[1], // subcollection name
                                    parts[2], // doc_id
                                    &fields,
                                    None::<Vec<String>>,
                                    None,
                                    vec![],
                                )
                                .map_err(to_firestore_error)?;
                        } else if parts.len() == 2 {
                            // Standard collection/doc_id
                            batch
                                .update_object(
                                    parts[0], // collection
                                    parts[1], // doc_id
                                    &fields,
                                    None::<Vec<String>>,
                                    None,
                                    vec![],
                                )
                                .map_err(to_firestore_error)?;
                        } else {
                            return Err(AppError::FirestoreError(format!(
                                "Invalid doc path format: expected 'collection/doc_id' or 'parent/subcol/doc', got '{}'",
                                doc_path
                            )));
                        }
                    }

                    batch.write().await.map_err(to_firestore_error)?;
                }

                Ok(())
            }
        }, &context).await
    }

    async fn batch_update_subcollection_docs(
        &self,
        parent_collection: &str,
        updates: Vec<(&str, &str, &str, HashMap<String, serde_json::Value>)>, // (parent_doc_id, subcollection, doc_id, fields)
    ) -> AppResult<()> {
        if updates.is_empty() {
            return Ok(());
        }

        // Firestore batch limit is 500 operations
        const BATCH_LIMIT: usize = 500;

        let batch_writer = self
            .db
            .create_simple_batch_writer()
            .await
            .map_err(to_firestore_error)?;

        // Split into batches if needed
        for chunk in updates.chunks(BATCH_LIMIT) {
            let mut batch = batch_writer.new_batch();

            for (parent_doc_id, subcollection, doc_id, fields) in chunk {
                // Use dot notation for subcollections: parent_collection.parent_doc_id.subcollection
                let collection_path =
                    format!("{}.{}.{}", parent_collection, parent_doc_id, subcollection);

                batch
                    .update_object(
                        &collection_path,
                        doc_id,
                        &fields,
                        None::<Vec<String>>,
                        None,
                        vec![],
                    )
                    .map_err(to_firestore_error)?;
            }

            batch.write().await.map_err(to_firestore_error)?;
        }

        Ok(())
    }

    async fn set_subcollection_doc<T: Serialize + DeserializeOwned + Send + Sync>(
        &self,
        parent_collection: &str,
        parent_doc_id: &str,
        subcollection: &str,
        doc_id: &str,
        data: &T,
    ) -> AppResult<()> {
        let mut fields = HashMap::new();
        let data_value = serde_json::to_value(data).map_err(to_firestore_error)?;
        if let serde_json::Value::Object(map) = data_value {
            fields.extend(map);
        } else {
            return Err(AppError::FirestoreError(
                "Expected token payload to serialize to a JSON object".to_string(),
            ));
        }

        let subcollection_doc_path = format!("{}/{}/{}", parent_doc_id, subcollection, doc_id);
        let update_result = self
            .update_doc(parent_collection, &subcollection_doc_path, fields.clone())
            .await;

        match update_result {
            Ok(()) => return Ok(()),
            Err(err) if !is_not_found_error(&err) => return Err(err),
            Err(_) => {}
        }

        // The firestore crate uses dot notation for subcollections
        // Format: parent_collection.parent_doc_id.subcollection
        let collection_path = format!("{}.{}.{}", parent_collection, parent_doc_id, subcollection);

        let create_result = self
            .db
            .fluent()
            .insert()
            .into(collection_path.as_str())
            .document_id(doc_id)
            .object(data)
            .execute::<()>()
            .await
            .map_err(to_firestore_error);

        match create_result {
            Ok(()) => Ok(()),
            Err(err) if is_already_exists_error(&err) => {
                self.update_doc(parent_collection, &subcollection_doc_path, fields)
                    .await
            }
            Err(err) => Err(err),
        }
    }

    async fn query_subcollection<T: DeserializeOwned + Send>(
        &self,
        parent_collection: &str,
        parent_doc_id: &str,
        subcollection: &str,
        filters: Vec<QueryFilter>,
        order_by: Option<&str>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> AppResult<Vec<T>> {
        let collection_path = format!("{}.{}.{}", parent_collection, parent_doc_id, subcollection);

        let mut query = self.db.fluent().select().from(collection_path.as_str());

        if !filters.is_empty() {
            query = query.filter(|q| {
                q.for_all(filters.iter().map(|f| {
                    let fv = json_to_firestore_value(&f.value);
                    match f.op {
                        QueryOp::Eq => q.field(f.field.as_str()).equal(fv),
                        QueryOp::NotEq => q.field(f.field.as_str()).not_equal(fv),
                        QueryOp::Gt => q.field(f.field.as_str()).greater_than(fv),
                        QueryOp::Lt => q.field(f.field.as_str()).less_than(fv),
                        QueryOp::GtEq => q.field(f.field.as_str()).greater_than_or_equal(fv),
                        QueryOp::LtEq => q.field(f.field.as_str()).less_than_or_equal(fv),
                        QueryOp::In => q.field(f.field.as_str()).is_in(fv),
                    }
                }))
            });
        }

        if let Some(order_field) = order_by {
            query = query.order_by([(order_field, FirestoreQueryDirection::Descending)]);
        }

        if let Some(l) = limit {
            query = query.limit(l);
        }

        if let Some(o) = offset {
            query = query.offset(o);
        }

        let results = query.obj().query().await.map_err(to_firestore_error)?;

        Ok(results)
    }

    async fn delete_subcollection_doc(
        &self,
        parent_collection: &str,
        parent_doc_id: &str,
        subcollection: &str,
        doc_id: &str,
    ) -> AppResult<()> {
        // Use dot notation for subcollections: parent_collection.parent_doc_id.subcollection
        let collection_path = format!("{}.{}.{}", parent_collection, parent_doc_id, subcollection);

        self.db
            .fluent()
            .delete()
            .from(collection_path.as_str())
            .document_id(doc_id)
            .execute()
            .await
            .map_err(to_firestore_error)?;
        Ok(())
    }

    async fn delete_doc(&self, collection: &str, doc_id: &str) -> AppResult<()> {
        self.db
            .fluent()
            .delete()
            .from(collection)
            .document_id(doc_id)
            .execute()
            .await
            .map_err(to_firestore_error)?;
        Ok(())
    }

    async fn query_collection<T: DeserializeOwned + Send>(
        &self,
        collection: &str,
        filters: Vec<QueryFilter>,
        order_by: Option<&str>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> AppResult<Vec<T>> {
        let mut query = self.db.fluent().select().from(collection);

        if !filters.is_empty() {
            query = query.filter(|q| {
                q.for_all(filters.iter().map(|f| {
                    let fv = json_to_firestore_value(&f.value);
                    match f.op {
                        QueryOp::Eq => q.field(f.field.as_str()).equal(fv),
                        QueryOp::NotEq => q.field(f.field.as_str()).not_equal(fv),
                        QueryOp::Gt => q.field(f.field.as_str()).greater_than(fv),
                        QueryOp::Lt => q.field(f.field.as_str()).less_than(fv),
                        QueryOp::GtEq => q.field(f.field.as_str()).greater_than_or_equal(fv),
                        QueryOp::LtEq => q.field(f.field.as_str()).less_than_or_equal(fv),
                        QueryOp::In => q.field(f.field.as_str()).is_in(fv),
                    }
                }))
            });
        }

        if let Some(order_field) = order_by {
            query = query.order_by([(order_field, FirestoreQueryDirection::Descending)]);
        }

        if let Some(l) = limit {
            query = query.limit(l);
        }

        if let Some(o) = offset {
            query = query.offset(o);
        }

        let results = query.obj().query().await.map_err(to_firestore_error)?;

        Ok(results)
    }

    async fn count_subcollection(
        &self,
        parent_collection: &str,
        parent_doc_id: &str,
        subcollection: &str,
        filters: Vec<QueryFilter>,
    ) -> AppResult<u32> {
        let collection_path = format!("{}.{}.{}", parent_collection, parent_doc_id, subcollection);

        let mut query = self.db.fluent().select().from(collection_path.as_str());

        if !filters.is_empty() {
            query = query.filter(|q| {
                q.for_all(filters.iter().map(|f| {
                    let fv = json_to_firestore_value(&f.value);
                    match f.op {
                        QueryOp::Eq => q.field(f.field.as_str()).equal(fv),
                        QueryOp::NotEq => q.field(f.field.as_str()).not_equal(fv),
                        QueryOp::Gt => q.field(f.field.as_str()).greater_than(fv),
                        QueryOp::Lt => q.field(f.field.as_str()).less_than(fv),
                        QueryOp::GtEq => q.field(f.field.as_str()).greater_than_or_equal(fv),
                        QueryOp::LtEq => q.field(f.field.as_str()).less_than_or_equal(fv),
                        QueryOp::In => q.field(f.field.as_str()).is_in(fv),
                    }
                }))
            });
        }

        #[derive(serde::Deserialize)]
        struct CountResult {
            count: i64,
        }

        let results: Vec<CountResult> = query
            .aggregate(|a| a.fields([a.field("count").count()]))
            .obj()
            .query()
            .await
            .map_err(to_firestore_error)?;

        Ok(results.first().map(|r| r.count as u32).unwrap_or(0))
    }

    async fn count_collection(
        &self,
        collection: &str,
        filters: Vec<QueryFilter>,
    ) -> AppResult<u32> {
        let mut query = self.db.fluent().select().from(collection);

        if !filters.is_empty() {
            query = query.filter(|q| {
                q.for_all(filters.iter().map(|f| {
                    let fv = json_to_firestore_value(&f.value);
                    match f.op {
                        QueryOp::Eq => q.field(f.field.as_str()).equal(fv),
                        QueryOp::NotEq => q.field(f.field.as_str()).not_equal(fv),
                        QueryOp::Gt => q.field(f.field.as_str()).greater_than(fv),
                        QueryOp::Lt => q.field(f.field.as_str()).less_than(fv),
                        QueryOp::GtEq => q.field(f.field.as_str()).greater_than_or_equal(fv),
                        QueryOp::LtEq => q.field(f.field.as_str()).less_than_or_equal(fv),
                        QueryOp::In => q.field(f.field.as_str()).is_in(fv),
                    }
                }))
            });
        }

        #[derive(serde::Deserialize)]
        struct CountResult {
            count: i64,
        }

        let results: Vec<CountResult> = query
            .aggregate(|a| a.fields([a.field("count").count()]))
            .obj()
            .query()
            .await
            .map_err(to_firestore_error)?;

        Ok(results.first().map(|r| r.count as u32).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::{is_already_exists_error, is_not_found_error, is_retryable_firestore_error};
    use crate::core::error::AppError;

    #[test]
    fn test_is_not_found_error_detects_firestore_not_found() {
        assert!(is_not_found_error(&AppError::FirestoreError(
            "Error code: NotFound".to_string()
        )));
        assert!(is_not_found_error(&AppError::FirestoreError(
            "document not found".to_string()
        )));
        assert!(!is_not_found_error(&AppError::FirestoreError(
            "Error code: AlreadyExists".to_string()
        )));
    }

    #[test]
    fn test_is_already_exists_error_detects_firestore_conflict() {
        assert!(is_already_exists_error(&AppError::FirestoreError(
            "Error code: AlreadyExists".to_string()
        )));
        assert!(is_already_exists_error(&AppError::FirestoreError(
            "Document already exists".to_string()
        )));
        assert!(!is_already_exists_error(&AppError::FirestoreError(
            "Error code: NotFound".to_string()
        )));
    }

    #[test]
    fn test_is_retryable_firestore_error() {
        // Retryable errors
        assert!(is_retryable_firestore_error(&AppError::FirestoreError(
            "UNAVAILABLE: Service temporarily unavailable".to_string()
        )));
        assert!(is_retryable_firestore_error(&AppError::FirestoreError(
            "DEADLINE_EXCEEDED".to_string()
        )));
        assert!(is_retryable_firestore_error(&AppError::FirestoreError(
            "RESOURCE_EXHAUSTED".to_string()
        )));
        assert!(is_retryable_firestore_error(&AppError::FirestoreError(
            "INTERNAL".to_string()
        )));
        assert!(is_retryable_firestore_error(&AppError::FirestoreError(
            "HTTP 503".to_string()
        )));
        assert!(is_retryable_firestore_error(&AppError::FirestoreError(
            "request timed out".to_string()
        )));

        // Non-retryable errors
        assert!(!is_retryable_firestore_error(&AppError::FirestoreError(
            "NotFound: document not found".to_string()
        )));
        assert!(!is_retryable_firestore_error(&AppError::FirestoreError(
            "PERMISSION_DENIED".to_string()
        )));
    }
}
