use std::path::Path;
use serde::Deserialize;

// ── Structure config.toml ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub provider: ProviderConfig,
    pub slots: SlotsConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    pub model: String,
}

#[derive(Debug, Deserialize)]
pub struct SlotsConfig {
    pub build: String,
    pub plan: String,
    pub chat: String,
    pub distill: String,
}

// ── Secrets (secrets.env) ────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Secrets {
    pub groq_api_key: String,
}

// ── Chargement ───────────────────────────────────────────────────────────────

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let path = Path::new("config.toml");
    let content = std::fs::read_to_string(path)
        .map_err(|_| "config.toml introuvable — lancez depuis la racine du repo")?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

pub fn load_secrets() -> Result<Secrets, Box<dyn std::error::Error>> {
    dotenvy::from_filename("secrets.env").ok();
    let key = std::env::var("GROQ_API_KEY")
        .map_err(|_| "GROQ_API_KEY absent de secrets.env")?;
    Ok(Secrets { groq_api_key: key })
}
