mod bootstrap;
mod core;
mod doctor_actor;
mod model;
mod module;
mod openapi;
mod repo;
mod syst;

pub use syst::config;

use clap::Parser;
use std::net::SocketAddr;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "server")]
#[command(about = "TDH Doctor App API Gateway", long_about = None)]
struct Args {
    #[arg(short, long)]
    config_dir: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let (cfg, tracer_provider) = bootstrap::init_infrastructure_with_config(args.config_dir)?;
    let mut deps = bootstrap::init_repos_and_services(&cfg).await?;
    let cancel_token = tokio_util::sync::CancellationToken::new();
    let routers = bootstrap::init_routers(&cfg, &mut deps).await?;

    // Warm up Redis ranking cache from PostgreSQL
    {
        let ranking_repo = deps.ranking_repo.clone();
        let ranking_cache = deps.ranking_cache.clone();
        tokio::spawn(async move {
            match ranking_repo.get_all_ranked_doctors().await {
                Ok(doctors) => ranking_cache.warm_up(&doctors).await,
                Err(e) => tracing::warn!(error = %e, "Ranking cache warm-up failed"),
            }
        });
    }

    let app = bootstrap::build_app(routers);

    let addr: SocketAddr = format!("{}:{}", cfg.http_server.host, cfg.http_server.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Server listening on {addr}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(bootstrap::shutdown_signal(tracer_provider, cancel_token))
    .await?;

    Ok(())
}
