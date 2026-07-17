//! Integration tests for doctor_id (UUID v7) generation during admin approval.
//! Uses testcontainers + the real migration chain (see tests/common).

mod common;

use sqlx::Row;

/// Insert a fully-populated PendingApproval draft for the given account.
async fn seed_pending_draft(pool: &sqlx::PgPool, account_id: i32) {
    sqlx::query(
        r#"
        INSERT INTO doctor_profile_draft
            (doctor_account_id, doctor_profile_id, citizen_id, license_number, address_detail,
             sub_district, district, province, postal_code,
             profile_image_url, id_card_image_url, book_bank_image_url, medical_license_image_url, status)
        VALUES
            ($1, 1, 'enc-token', 'L123', 'addr',
             '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, 10200,
             'u', 'u', 'u', 'u', 'PendingApproval'::doctor_profile_status_enum)
        "#,
    )
    .bind(account_id)
    .execute(pool)
    .await
    .expect("seed draft");
}

async fn approve(pool: &sqlx::PgPool, account_id: i32) {
    // citizen_id ciphertext lives in the draft (encrypted at save); approval copies it.
    // We pass the KMS key name literal since we exercise the SQL function directly.
    sqlx::query("SELECT approve_doctor_profile_draft($1, $2, $3)")
        .bind(account_id)
        .bind(42_i32)
        .bind("projects/p/locations/l/keyRings/r/cryptoKeys/k")
        .execute(pool)
        .await
        .expect("approve");
}

async fn doctor_id_text(pool: &sqlx::PgPool, account_id: i32) -> String {
    sqlx::query("SELECT doctor_id::text AS id FROM doctor_profile WHERE doctor_account_id = $1")
        .bind(account_id)
        .fetch_one(pool)
        .await
        .expect("fetch doctor_id")
        .get::<String, _>("id")
}

#[tokio::test]
async fn approval_generates_a_uuid_v7_doctor_id() {
    let (_pg, pool) = common::setup_postgres().await;
    let account_id = 9001;

    seed_pending_draft(&pool, account_id).await;
    approve(&pool, account_id).await;

    let id = doctor_id_text(&pool, account_id).await;
    assert!(!id.is_empty(), "doctor_id should be populated");
    // UUID text: xxxxxxxx-xxxx-Vxxx-...  — char 15 (1-based) is the version nibble.
    assert_eq!(&id[14..15], "7", "doctor_id must be UUID v7, got {id}");
}

#[tokio::test]
async fn re_approval_preserves_the_original_doctor_id() {
    let (_pg, pool) = common::setup_postgres().await;
    let account_id = 9002;

    seed_pending_draft(&pool, account_id).await;
    approve(&pool, account_id).await;
    let first = doctor_id_text(&pool, account_id).await;

    // Simulate reject -> edit -> resubmit -> re-approve.
    sqlx::query(
        "UPDATE doctor_profile_draft SET status = 'PendingApproval'::doctor_profile_status_enum \
         WHERE doctor_account_id = $1",
    )
    .bind(account_id)
    .execute(&pool)
    .await
    .expect("reset draft to pending");

    approve(&pool, account_id).await;
    let second = doctor_id_text(&pool, account_id).await;

    assert_eq!(
        first, second,
        "re-approval must preserve the original doctor_id"
    );
}

#[tokio::test]
async fn doctor_id_is_immutable() {
    let (_pg, pool) = common::setup_postgres().await;
    let account_id = 9003;

    seed_pending_draft(&pool, account_id).await;
    approve(&pool, account_id).await;

    let result = sqlx::query(
        "UPDATE doctor_profile \
         SET doctor_id = '00000000-0000-7000-8000-000000000000' \
         WHERE doctor_account_id = $1",
    )
    .bind(account_id)
    .execute(&pool)
    .await;

    assert!(
        result.is_err(),
        "changing doctor_id must be rejected by the immutability trigger"
    );
}
