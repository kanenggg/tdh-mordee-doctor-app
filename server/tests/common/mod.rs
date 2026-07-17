use async_trait::async_trait;
use server::core::error::AppResult;
use server::core::kms::KmsClient;
use sqlx::PgPool;
use testcontainers::core::IntoContainerPort;
use testcontainers::{runners::AsyncRunner, ContainerAsync, ImageExt};
use testcontainers_modules::postgres::Postgres;

/// In-memory KMS stub for tests: `encrypt` prefixes `enc:`, `decrypt` strips it.
pub struct MockKms;

#[async_trait]
impl KmsClient for MockKms {
    async fn encrypt(&self, plaintext: &str) -> AppResult<String> {
        Ok(format!("enc:{plaintext}"))
    }
    async fn decrypt(&self, ciphertext: &str) -> AppResult<String> {
        Ok(ciphertext
            .strip_prefix("enc:")
            .unwrap_or(ciphertext)
            .to_string())
    }
    fn key_name(&self) -> &str {
        "test-kms-key"
    }
}

pub async fn setup_postgres() -> (ContainerAsync<Postgres>, PgPool) {
    // The module does not declare an exposed port. Map it explicitly so
    // Testcontainers works with DOCKER_HOST=rootless Podman as well as Docker.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("reserve a free Postgres host port")
        .local_addr()
        .unwrap()
        .port();
    let container = Postgres::default()
        .with_mapped_port(port, 5432.tcp())
        .start()
        .await
        .expect("start PostgreSQL Testcontainer");
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);
    let pool = PgPool::connect(&database_url).await.unwrap();

    run_all_migrations(&pool).await;

    (container, pool)
}

async fn run_all_migrations(pool: &PgPool) {
    sqlx::raw_sql("CREATE EXTENSION IF NOT EXISTS pgcrypto")
        .execute(pool)
        .await
        .expect("failed to enable pgcrypto extension for test database");

    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("db")
        .join("postgres")
        .join("migrations");

    let mut migrations = std::fs::read_dir(&migrations_dir)
        .unwrap_or_else(|e| panic!("failed to read migrations dir {migrations_dir:?}: {e}"))
        .map(|entry| entry.expect("failed to read migration dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .collect::<Vec<_>>();

    migrations.sort();

    for migration in migrations {
        let sql = std::fs::read_to_string(&migration)
            .unwrap_or_else(|e| panic!("failed to read migration {migration:?}: {e}"));

        sqlx::raw_sql(&sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("failed to run migration {migration:?}: {e}"));
    }
}
