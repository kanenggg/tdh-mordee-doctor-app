use clap::Parser;
use deadpool_redis::{redis::AsyncCommands, Config as RedisConfig, Runtime};
use dotenvy::dotenv;
use server::config::AppConfig;
use server::doctor_actor::model::DoctorScheduleConfig;
use std::io::{IsTerminal, Read};
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Set or update a doctor's schedule config in Redis.
///
/// Reads schedule JSON from a file, inline argument, or stdin.
///
/// Redis key format: doctor:{doctor_id}:schedule_config
#[derive(Parser)]
#[command(name = "set_doctor_schedule")]
struct Cli {
    /// Doctor ID
    doctor_id: String,

    /// Redis URL (overrides config)
    #[arg(long)]
    redis_url: Option<String>,

    /// Inline JSON schedule (alternative to --file or stdin)
    #[arg(long)]
    json: Option<String>,

    /// Path to a JSON file containing the schedule
    #[arg(long)]
    file: Option<String>,

    /// Only show the current schedule without setting a new one
    #[arg(long)]
    show: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("set_doctor_schedule=info".parse()?),
        )
        .init();

    let cli = Cli::parse();
    let key = format!("doctor:{}:schedule_config", cli.doctor_id);

    // Resolve Redis URL: CLI flag > env/config
    let redis_url = match cli.redis_url {
        Some(url) => url,
        None => {
            let cfg = AppConfig::load_from_dir(None)?;
            cfg.redis.url
        }
    };

    let pool = RedisConfig::from_url(&redis_url).create_pool(Some(Runtime::Tokio1))?;
    let mut conn = pool.get().await?;

    // Show current schedule
    let current: Option<String> = conn.get(&key).await?;
    match &current {
        Some(json) => {
            let pretty: serde_json::Value = serde_json::from_str(json)?;
            info!("Current schedule for doctor {}:", cli.doctor_id);
            println!("{}", serde_json::to_string_pretty(&pretty)?);
        }
        None => {
            info!("No existing schedule for doctor {}.", cli.doctor_id);
        }
    }

    if cli.show {
        return Ok(());
    }

    // Read schedule JSON from one of the sources
    let input_json = if let Some(json) = cli.json {
        json
    } else if let Some(path) = cli.file {
        std::fs::read_to_string(&path)?
    } else if std::io::stdin().is_terminal() {
        anyhow::bail!(
            "No schedule JSON provided. Use --json, --file, or pipe JSON to stdin.\n\
             Example: cargo run --bin set_doctor_schedule -- 123 --file schedule.json"
        );
    } else {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    };

    // Validate by deserializing into the domain type
    let config: DoctorScheduleConfig = serde_json::from_str(&input_json)
        .map_err(|e| anyhow::anyhow!("Invalid schedule JSON: {e}"))?;

    let canonical = serde_json::to_string(&config)?;
    let _: () = conn.set(&key, &canonical).await?;

    info!("Schedule saved for doctor {} (key: {})", cli.doctor_id, key);
    println!("{}", serde_json::to_string_pretty(&config)?);

    Ok(())
}
