use server_bg::module::doctor_profile_outbox::{
    DoctorProfileOutboxRepo, PostgresDoctorProfileOutboxRepo,
};
use sqlx::PgPool;
use testcontainers::core::IntoContainerPort;
use testcontainers::{runners::AsyncRunner, ContainerAsync, ImageExt};
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

async fn setup() -> (ContainerAsync<Postgres>, PgPool) {
    // AsyncRunner discovers DOCKER_HOST itself, including rootless Podman unix
    // sockets. Do not preflight /var/run/docker.sock: that wrongly skips CI.
    let host_port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("reserve a free host port for Postgres Testcontainer")
        .local_addr()
        .unwrap()
        .port();
    let container = Postgres::default()
        // Postgres module metadata does not expose 5432. Explicit mapping also
        // works through rootless Podman's DOCKER_HOST API.
        .with_mapped_port(host_port, 5432.tcp())
        .start()
        .await
        .expect("Postgres Testcontainers requires a reachable Docker/Podman daemon; set DOCKER_HOST or run this explicitly in CI");
    let pool = PgPool::connect(&format!(
        "postgres://postgres:postgres@127.0.0.1:{host_port}/postgres"
    ))
    .await
    .expect("connect to Postgres Testcontainer");
    sqlx::raw_sql(r#"
        CREATE TABLE doctor_profile (doctor_id uuid PRIMARY KEY);
        CREATE TABLE doctor_profile_event_outbox (
          event_id uuid PRIMARY KEY, doctor_id uuid NOT NULL REFERENCES doctor_profile(doctor_id),
          doctor_account_id integer NOT NULL, event_type text NOT NULL, schema_version integer NOT NULL,
          profile_version bigint NOT NULL, occurred_at timestamptz NOT NULL, payload jsonb NOT NULL,
          attempts integer NOT NULL DEFAULT 0, available_at timestamptz NOT NULL DEFAULT now(),
          lease_token uuid, leased_until timestamptz, published_at timestamptz, last_error text,
          created_at timestamptz NOT NULL DEFAULT now(), UNIQUE (doctor_id, profile_version)
        );
    "#).execute(&pool).await.unwrap();
    (container, pool)
}

async fn insert_doctor(pool: &PgPool) -> Uuid {
    let doctor_id = Uuid::new_v4();
    sqlx::query("INSERT INTO doctor_profile (doctor_id) VALUES ($1)")
        .bind(doctor_id)
        .execute(pool)
        .await
        .expect("insert doctor");
    doctor_id
}

async fn insert_event(pool: &PgPool, doctor_id: Uuid, account: i32, version: i64) -> Uuid {
    let event_id = Uuid::new_v4();
    sqlx::query(r#"INSERT INTO doctor_profile_event_outbox
       (event_id, doctor_id, doctor_account_id, event_type, schema_version, profile_version, occurred_at, payload)
       VALUES ($1,$2,$3,'DoctorProfileApproved',2,$4,now(),'{}')"#)
        .bind(event_id).bind(doctor_id).bind(account).bind(version).execute(pool).await.unwrap();
    event_id
}

#[tokio::test]
async fn concurrent_leases_skip_locked_rows() {
    let (_container, pool) = setup().await;
    let left_doctor = insert_doctor(&pool).await;
    let right_doctor = insert_doctor(&pool).await;
    insert_event(&pool, left_doctor, 1, 1).await;
    insert_event(&pool, right_doctor, 2, 1).await;
    let repo = PostgresDoctorProfileOutboxRepo::new(pool.clone());
    let (left, right) = tokio::join!(repo.lease_ready(1, 30), repo.lease_ready(1, 30));
    let left = left.unwrap();
    let right = right.unwrap();
    assert_eq!(left.len() + right.len(), 2);
    assert_ne!(left[0].event_id, right[0].event_id);
}

#[tokio::test]
async fn expired_lease_can_be_reclaimed_and_stale_token_is_rejected() {
    let (_container, pool) = setup().await;
    let doctor = insert_doctor(&pool).await;
    insert_event(&pool, doctor, 1, 1).await;
    let repo = PostgresDoctorProfileOutboxRepo::new(pool.clone());
    let old = repo.lease_ready(1, 1).await.unwrap().pop().unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let fresh = repo.lease_ready(1, 30).await.unwrap().pop().unwrap();
    assert_ne!(old.lease_token, fresh.lease_token);
    assert!(repo
        .mark_published(old.event_id, old.lease_token)
        .await
        .is_err());
    repo.reschedule(fresh.event_id, fresh.lease_token, 30)
        .await
        .unwrap();
    let state = sqlx::query(
        "SELECT lease_token, leased_until IS NULL AS lease_cleared, last_error FROM doctor_profile_event_outbox WHERE event_id = $1",
    ).bind(fresh.event_id).fetch_one(&pool).await.unwrap();
    use sqlx::Row;
    assert_eq!(state.get::<Option<Uuid>, _>("lease_token"), None);
    assert!(state.get::<bool, _>("lease_cleared"));
    assert_eq!(state.get::<String, _>("last_error"), "publish failed");
}

#[tokio::test]
async fn later_versions_remain_blocked_while_oldest_is_leased_or_retrying() {
    let (_container, pool) = setup().await;
    let doctor = insert_doctor(&pool).await;
    let first_id = insert_event(&pool, doctor, 7, 1).await;
    let second_id = insert_event(&pool, doctor, 7, 2).await;
    let first_worker = PostgresDoctorProfileOutboxRepo::new(pool.clone());
    let second_worker = PostgresDoctorProfileOutboxRepo::new(pool.clone());

    let first = first_worker.lease_ready(10, 30).await.unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].event_id, first_id);
    assert!(second_worker.lease_ready(10, 30).await.unwrap().is_empty());

    first_worker
        .reschedule(first_id, first[0].lease_token, 60)
        .await
        .unwrap();
    assert!(second_worker.lease_ready(10, 30).await.unwrap().is_empty());

    sqlx::query("UPDATE doctor_profile_event_outbox SET available_at = now() WHERE event_id = $1")
        .bind(first_id)
        .execute(&pool)
        .await
        .unwrap();
    let retry = first_worker.lease_ready(10, 30).await.unwrap();
    assert_eq!(retry.len(), 1);
    assert_eq!(retry[0].event_id, first_id);
    first_worker
        .mark_published(first_id, retry[0].lease_token)
        .await
        .unwrap();

    let next = second_worker.lease_ready(10, 30).await.unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].event_id, second_id);
}
