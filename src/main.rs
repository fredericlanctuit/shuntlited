mod config;
mod models;
mod providers;
mod router;

use std::sync::Arc;
use axum::{routing::{get, post}, Router};
use tracing::info;
use tracing_subscriber::EnvFilter;
use router::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // -- Tracing

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("shuntlited=info".parse()?))
        .init();

    // -- Config + secrets

    let mut cfg = config::load_config()?;
    let secrets = config::load_secrets()?;

    config::log_startup(&cfg, &secrets);

    cfg.secrets = secrets;

    let bind = format!("{}:{}", cfg.server.host, cfg.server.port);

    // -- Sled : quota state + cooldowns

    let db_path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".shuntlited/state/quota.sled");

    let db = sled::open(&db_path)?;
    info!(path = %db_path.display(), "Sled ouvert");

    // -- Etat partage

    let state = Arc::new(AppState {
        config: cfg,
        http: reqwest::Client::new(),
        db,
    });

    // -- Routes

    let app = Router::new()
        .route("/ready",               get(router::health_check))
        .route("/v1/chat/completions", post(router::chat_completions))
        .with_state(state);

    // -- Bind

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    info!(addr = %bind, "En ecoute");
    axum::serve(listener, app).await?;
    Ok(())
}
