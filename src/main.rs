mod config;
mod models;
mod router;

use std::sync::Arc;
use axum::{routing::{get, post}, Router};
use tracing::info;
use tracing_subscriber::EnvFilter;

use router::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // ── Tracing ──────────────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("shuntlited=info".parse()?))
        .init();

    // ── Config + secrets ─────────────────────────────────────────────────────
    let cfg     = config::load_config()?;
    let secrets = config::load_secrets()?;
    let bind    = format!("{}:{}", cfg.server.host, cfg.server.port);

    info!(
        host = %cfg.server.host,
        port = %cfg.server.port,
        provider = %cfg.provider.name,
        model = %cfg.provider.model,
        "ShuntLited démarrage"
    );

    // ── État partagé ─────────────────────────────────────────────────────────
    let state = Arc::new(AppState {
        config: cfg,
        secrets,
        http: reqwest::Client::new(),
    });

    // ── Routes ───────────────────────────────────────────────────────────────
    let app = Router::new()
        .route("/ready",               get(router::health_check))
        .route("/v1/chat/completions", post(router::chat_completions))
        .with_state(state);

    // ── Bind ─────────────────────────────────────────────────────────────────
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    info!(addr = %bind, "En écoute");
    axum::serve(listener, app).await?;

    Ok(())
}
