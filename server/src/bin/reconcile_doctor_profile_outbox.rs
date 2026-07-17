//! Emits V2 snapshots for already-approved doctors missed before the outbox rollout.
use clap::Parser;
use server::module::profile_event_outbox::{reconcile_approved_doctors, ReconciliationResult};
use sqlx::postgres::PgPoolOptions;

#[derive(Parser)]
#[command(about = "Backfill DoctorProfile V2 outbox snapshots")]
struct Args {
    #[arg(short, long, default_value = "./server/config")]
    config_dir: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let args = Args::parse();
    let cfg = server::config::AppConfig::load_from_dir(Some(args.config_dir))?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.postgres.database_url())
        .await?;
    let result = reconcile_approved_doctors(&pool).await?;
    println!(
        "doctor profile outbox reconciliation complete: emitted={} skipped={} failures={}",
        result.emitted, result.skipped, result.failures
    );
    ensure_reconciliation_succeeded(result)
}

fn ensure_reconciliation_succeeded(result: ReconciliationResult) -> anyhow::Result<()> {
    if result.failures > 0 {
        anyhow::bail!(
            "doctor profile outbox reconciliation failed for {} profile(s)",
            result.failures
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconciliation_with_profile_failures_returns_an_error() {
        let result = ReconciliationResult {
            emitted: 1,
            skipped: 2,
            failures: 1,
        };

        assert!(ensure_reconciliation_succeeded(result).is_err());
    }
}
