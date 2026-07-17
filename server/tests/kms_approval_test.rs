//! Verifies citizen_id is KMS-encrypted at SAVE (Option B):
//! draft holds ciphertext -> owner read decrypts -> approval copies ciphertext through.

mod common;

use std::sync::Arc;

use server::model::onboarding::OnBoardingRequest;
use server::module::onboarding::repo::{OnBoardingRepo, OnBoardingRepoImp};
use sqlx::Row;

#[tokio::test]
async fn save_encrypts_and_approval_copies_ciphertext() {
    let (_pg, pool) = common::setup_postgres().await;
    let account_id = 7001;
    let plaintext = "1101700203450";

    let repo = OnBoardingRepoImp::new(pool.clone(), Arc::new(common::MockKms));

    // 1. Save draft -> citizen_id is KMS-encrypted at save time.
    let request = OnBoardingRequest {
        citizen_id: plaintext.to_string(),
        ..Default::default()
    };
    repo.save_doctor_profile_draft(account_id, 1, &request)
        .await
        .expect("save draft");

    // The draft column holds ciphertext, never the plaintext.
    let draft_ct: Option<String> =
        sqlx::query("SELECT citizen_id FROM doctor_profile_draft WHERE doctor_account_id = $1")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("citizen_id");
    assert_eq!(draft_ct.as_deref(), Some("enc:1101700203450"));

    // Owner draft-read goes through get_doctor_profile_draft (citizen_id text) and
    // KMS-decrypts the ciphertext back to plaintext for the owner.
    let stub = repo
        .get_doctor_profile_draft(account_id)
        .await
        .expect("read draft")
        .expect("draft exists");
    assert_eq!(stub.citizen_id, plaintext);

    // 2. Approval copies the ciphertext through and records the KMS key (no re-encryption).
    sqlx::query(
        "UPDATE doctor_profile_draft SET status = 'PendingApproval'::doctor_profile_status_enum \
         WHERE doctor_account_id = $1",
    )
    .bind(account_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("SELECT approve_doctor_profile_draft($1, $2, $3)")
        .bind(account_id)
        .bind(42_i32)
        .bind("test-kms-key")
        .execute(&pool)
        .await
        .expect("approve");

    let row = sqlx::query(
        "SELECT citizen_id, citizen_id_kms_key FROM doctor_profile WHERE doctor_account_id = $1",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .expect("fetch profile");

    let stored: String = row.get("citizen_id");
    let kms_key: String = row.get("citizen_id_kms_key");

    // Same ciphertext as the draft (copied, not re-encrypted); never the plaintext.
    assert_eq!(stored, "enc:1101700203450");
    assert_ne!(stored, plaintext);
    assert_eq!(kms_key, "test-kms-key");
}
