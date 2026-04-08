use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use tracing::{error, info, warn};
use crate::config::{AppConfig, effective_providers};
use crate::models::{ChatRequest, ChatResponse, ErrorResponse, UpstreamRequest};
use crate::providers::get_provider_meta;

// -- Etat partage injecte par Axum

pub struct AppState {
    pub config: AppConfig,
    pub http: reqwest::Client,
    pub db: sled::Db,
}

// -- Cle sled pour cooldown provider

fn cooldown_key(provider: &str) -> String {
    format!("cooldown:{}", provider)
}

// -- Verifie si un provider est en cooldown

fn is_in_cooldown(db: &sled::Db, provider: &str) -> bool {
    let key = cooldown_key(provider);
    match db.get(key.as_bytes()) {
        Ok(Some(val)) => {
            let bytes: [u8; 8] = match val.as_ref().try_into() {
                Ok(b) => b,
                Err(_) => return false,
            };
            let until = u64::from_be_bytes(bytes);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            now < until
        }
        _ => false,
    }
}

// -- Ecrit un cooldown en sled (until = now + duree_secondes)

fn set_cooldown(db: &sled::Db, provider: &str, duration_secs: u64) {
    let key = cooldown_key(provider);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let until = now + duration_secs;
    let _ = db.insert(key.as_bytes(), &until.to_be_bytes());
}

// -- Handler POST /v1/chat/completions

pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    let slot = &req.model;

    let slot_config = match state.config.slots.get(slot) {
        Some(s) => s,
        None => {
            warn!(slot = %slot, "Slot inconnu");
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    format!("Slot inconnu : {slot}"),
                    "no_provider_available",
                    None,
                    None,
                )),
            ).into_response();
        }
    };

    let providers = effective_providers(slot_config, &state.config.secrets);

    if providers.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse::new(
                "Aucun provider actif pour ce slot.".to_string(),
                "no_provider_available",
                None,
                None,
            )),
        ).into_response();
    }

    let mut last_retry_after: Option<u64> = None;

    for provider_name in &providers {

        // -- Verifier cooldown sled

        if is_in_cooldown(&state.db, provider_name) {
            warn!(provider = %provider_name, "En cooldown — ignore");
            continue;
        }

        let meta = match get_provider_meta(provider_name) {
            Some(m) => m,
            None => {
                warn!(provider = %provider_name, "Provider sans meta — ignore");
                continue;
            }
        };

        let api_key = match state.config.secrets.get_key(provider_name) {
            Some(k) => k.clone(),
            None => continue,
        };

        let model = meta.default_model.to_string();

        info!(
            slot = %slot,
            provider = %provider_name,
            model = %model,
            "-> requete entrante"
        );

        let upstream = UpstreamRequest {
            model: &model,
            messages: &req.messages,
            max_tokens: req.max_tokens,
            temperature: req.temperature,
        };

        let url = format!("{}/chat/completions", meta.base_url.trim_end_matches('/'));

        let result = state
            .http
            .post(&url)
            .bearer_auth(&api_key)
            .json(&upstream)
            .send()
            .await;

        match result {
            Err(e) => {
                error!(provider = %provider_name, error = %e, "Echec connexion — essai suivant");
                continue;
            }
            Ok(resp) => {
                let status = resp.status();

                if status.as_u16() == 429 {
                    // -- Lire retry-after, appliquer plancher 300s

                    let retry_after = resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(300);

                    let cooldown = retry_after.max(300);
                    set_cooldown(&state.db, provider_name, cooldown);
                    last_retry_after = Some(cooldown);

                    warn!(
                        provider = %provider_name,
                        cooldown_secs = cooldown,
                        "429 recu — cooldown enregistre, passage au suivant"
                    );
                    continue;
                }

                if !status.is_success() {
                    let body = resp.text().await.unwrap_or_default();
                    error!(
                        provider = %provider_name,
                        status = %status,
                        body = %body,
                        "Erreur provider — essai suivant"
                    );
                    continue;
                }

                match resp.json::<ChatResponse>().await {
                    Ok(chat_resp) => {
                        info!(
                            provider = %provider_name,
                            model = %chat_resp.model,
                            tokens = ?chat_resp.usage.as_ref().map(|u| u.total_tokens),
                            "<- reponse recue"
                        );
                        return (StatusCode::OK, Json(chat_resp)).into_response();
                    }
                    Err(e) => {
                        error!(provider = %provider_name, error = %e, "Echec deserialisation");
                        continue;
                    }
                }
            }
        }
    }

    // -- Tous les providers ont echoue ou sont en cooldown

    let message = if let Some(secs) = last_retry_after {
        let minutes = secs / 60;
        format!(
            "Tous les providers sont indisponibles ou en cooldown. Reessayez dans ~{}min.",
            minutes
        )
    } else {
        "Tous les providers sont indisponibles. Verifiez vos cles API.".to_string()
    };

    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse::new(
            message,
            "quota_exhausted",
            None,
            last_retry_after,
        )),
    ).into_response()
}

// -- Handler GET /ready

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}
