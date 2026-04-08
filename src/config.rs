use std::collections::{HashMap, HashSet};
use std::path::Path;
use serde::Deserialize;

// ── Structures config.toml ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub slots: HashMap<String, SlotConfig>,
    #[serde(skip)]
    pub secrets: Secrets,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct SlotConfig {
    pub providers: Vec<String>,
}

// ── Providers sans free tier (exclus du routing automatique) ─────────────────

const NO_FREE_TIER: &[&str] = &["anthropic", "openai", "cohere"];

// ── Secrets (secrets.env) ────────────────────────────────────────────────────

/// Clés connues : nom_provider → variable d'environnement
const KNOWN_PROVIDERS: &[(&str, &str)] = &[
    ("groq",        "GROQ_API_KEY"),
    ("cerebras",    "CEREBRAS_API_KEY"),
    ("sambanova",   "SAMBANOVA_API_KEY"),
    ("openrouter",  "OPENROUTER_API_KEY"),
    ("mistral",     "MISTRAL_API_KEY"),
    ("scaleway",    "SCALEWAY_API_KEY"),
];

#[derive(Debug, Default)]
pub struct Secrets {
    pub active_providers: HashSet<String>,
    pub api_keys: HashMap<String, String>,
}

impl Secrets {
    pub fn get_key(&self, provider: &str) -> Option<&String> {
        self.api_keys.get(provider)
    }
}

// ── Chargement ───────────────────────────────────────────────────────────────

pub fn load_config() -> Result<AppConfig, Box<dyn std::error::Error>> {
    let path = Path::new("config.toml");
    let content = std::fs::read_to_string(path)
        .map_err(|_| "config.toml introuvable — lancez depuis la racine du repo")?;
    let config: AppConfig = toml::from_str(&content)?;
    Ok(config)
}

pub fn load_secrets() -> Result<Secrets, Box<dyn std::error::Error>> {
    dotenvy::from_filename("secrets.env").ok();

    let mut active_providers = HashSet::new();
    let mut api_keys = HashMap::new();

    for (provider, env_var) in KNOWN_PROVIDERS {
        if let Ok(key) = std::env::var(env_var) {
            if !key.trim().is_empty() {
                active_providers.insert(provider.to_string());
                api_keys.insert(provider.to_string(), key);
            }
        }
    }

    Ok(Secrets { active_providers, api_keys })
}

/// Retourne la liste effective d'un slot : providers déclarés ∩ clés présentes
pub fn effective_providers(
    slot_config: &SlotConfig,
    secrets: &Secrets,
) -> Vec<String> {
    slot_config.providers
        .iter()
        .filter(|p| secrets.active_providers.contains(*p))
        .cloned()
        .collect()
}

/// Log au démarrage : providers actifs + warnings sans free tier
pub fn log_startup(config: &AppConfig, secrets: &Secrets) {
    tracing::info!("Providers actifs : {:?}", secrets.active_providers);

    for provider in &secrets.active_providers {
        if NO_FREE_TIER.contains(&provider.as_str()) {
            tracing::warn!(
                "{}: pas de free tier — exclu du routing automatique",
                provider
            );
        }
    }

    for (slot_name, slot_cfg) in &config.slots {
        let effective = effective_providers(slot_cfg, secrets);
        if effective.is_empty() {
            tracing::warn!("Slot {} : aucun provider actif", slot_name);
        } else {
            tracing::info!("Slot {} : {:?}", slot_name, effective);
        }
    }
}
