use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use tracing::{error, info};

use crate::config::{Config, Secrets};
use crate::models::{ChatRequest, ChatResponse, ErrorResponse, UpstreamRequest};

// ── État partagé injecté par Axum ────────────────────────────────────────────

pub struct AppState {
    pub config: Config,
    pub secrets: Secrets,
    pub http: reqwest::Client,
}

// ── Handler POST /v1/chat/completions ────────────────────────────────────────

pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    let model = resolve_model(&req.model, &state.config);

    info!(
        slot = %req.model,
        model = %model,
        provider = %state.config.provider.name,
        "→ requête entrante"
    );

    let upstream = UpstreamRequest {
        model: &model,
        messages: &req.messages,
        max_tokens: req.max_tokens,
        temperature: req.temperature,
    };

    let url = format!(
        "{}/chat/completions",
        state.config.provider.base_url.trim_end_matches('/')
    );

    let result = state
        .http
        .post(&url)
        .bearer_auth(&state.secrets.groq_api_key)
        .json(&upstream)
        .send()
        .await;

    match result {
        Err(e) => {
            error!(error = %e, "Échec connexion provider");
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse::new(
                    format!("Provider injoignable : {e}"),
                    "provider_error",
                )),
            )
                .into_response()
        }
        Ok(resp) => {
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                error!(status = %status, body = %body, "Erreur provider");
                return (
                    StatusCode::from_u16(status.as_u16())
                        .unwrap_or(StatusCode::BAD_GATEWAY),
                    Json(ErrorResponse::new(body, "upstream_error")),
                )
                    .into_response();
            }

            match resp.json::<ChatResponse>().await {
                Ok(chat_resp) => {
                    info!(
                        model = %chat_resp.model,
                        tokens = ?chat_resp.usage.as_ref().map(|u| u.total_tokens),
                        "← réponse reçue"
                    );
                    (StatusCode::OK, Json(chat_resp)).into_response()
                }
                Err(e) => {
                    error!(error = %e, "Échec désérialisation réponse");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse::new(
                            format!("Réponse provider invalide : {e}"),
                            "parse_error",
                        )),
                    )
                        .into_response()
                }
            }
        }
    }
}

// ── Résolution slot → modèle réel ────────────────────────────────────────────

fn resolve_model<'a>(requested: &'a str, config: &'a Config) -> String {
    match requested {
        "gpt-build"   => config.slots.build.clone(),
        "gpt-plan"    => config.slots.plan.clone(),
        "gpt-chat"    => config.slots.chat.clone(),
        "gpt-distill" => config.slots.distill.clone(),
        other         => other.to_string(),
    }
}

// ── Handler GET /ready ───────────────────────────────────────────────────────

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}
