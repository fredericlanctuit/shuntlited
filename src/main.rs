mod config;
mod models;
mod providers;
mod router;
mod savings;

use std::sync::Arc;
use axum::{routing::{get, post}, Router, response::Json as AxumJson};
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

    // -- Bootstrap reference_pricing.json si absent
    savings::ensure_reference_pricing();

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
        .route("/data/routing.json",   get(routing_json_handler))
        .with_state(state);

    // -- Bind

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    info!(addr = %bind, "En ecoute");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn routing_json_handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> impl axum::response::IntoResponse {
    let active: Vec<String> = state.config.secrets.active_providers
        .iter().cloned().collect();
    let routing = savings::generate_routing_json(&state.db, &active);
    AxumJson(routing)
}
