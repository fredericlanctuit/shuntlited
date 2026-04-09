use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::warn;

// -- Chemins fichiers

pub fn data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".shuntlited/data")
}

pub fn routing_json_path() -> PathBuf {
    data_dir().join("routing.json")
}

pub fn reference_pricing_path() -> PathBuf {
    data_dir().join("reference_pricing.json")
}

// -- Cles sled quota

pub fn quota_rpd_key(provider: &str) -> String {
    format!("quota:rpd:{}", provider)
}

pub fn quota_rpm_key(provider: &str) -> String {
    format!("quota:rpm:{}", provider)
}

pub fn quota_tpd_key(provider: &str) -> String {
    format!("quota:tpd:{}", provider)
}

pub fn savings_total_key() -> &'static str {
    "savings:total_usd_micro"
}

pub fn savings_tokens_key() -> &'static str {
    "savings:total_tokens"
}

// -- Ecriture quota dans sled

pub fn write_quota(db: &sled::Db, provider: &str, rpd: Option<u64>, rpm: Option<u64>, tpd: Option<u64>) {
    if let Some(v) = rpd {
        let _ = db.insert(quota_rpd_key(provider).as_bytes(), &v.to_be_bytes());
    }
    if let Some(v) = rpm {
        let _ = db.insert(quota_rpm_key(provider).as_bytes(), &v.to_be_bytes());
    }
    if let Some(v) = tpd {
        let _ = db.insert(quota_tpd_key(provider).as_bytes(), &v.to_be_bytes());
    }
}

// -- Lecture u64 depuis sled

fn read_u64(db: &sled::Db, key: &str) -> Option<u64> {
    db.get(key.as_bytes()).ok()?.map(|v| {
        let bytes: [u8; 8] = v.as_ref().try_into().ok()?;
        Some(u64::from_be_bytes(bytes))
    })?
}

// -- Accumulation savings

pub fn accumulate_savings(db: &sled::Db, prompt_tokens: u32, completion_tokens: u32, provider: &str, model: &str) {
    let pricing = load_reference_pricing();
    let mapping = pricing.mappings.iter().find(|m| m.provider == provider && m.free_model == model);

    let (input_cost, output_cost) = match mapping {
        Some(m) => (m.input_cost_per_1m, m.output_cost_per_1m),
        None => {
            // Fallback : tarif generique si modele non trouve
            (3.0_f64, 15.0_f64)
        }
    };

    let saving_usd = (prompt_tokens as f64 * input_cost / 1_000_000.0)
        + (completion_tokens as f64 * output_cost / 1_000_000.0);

    // Stocker en micro-dollars (u64) pour eviter float dans sled
    let saving_micro = (saving_usd * 1_000_000.0) as u64;
    let total_tokens = (prompt_tokens + completion_tokens) as u64;

    let prev_micro = read_u64(db, savings_total_key()).unwrap_or(0);
    let prev_tokens = read_u64(db, savings_tokens_key()).unwrap_or(0);

    let _ = db.insert(savings_total_key().as_bytes(), &(prev_micro + saving_micro).to_be_bytes());
    let _ = db.insert(savings_tokens_key().as_bytes(), &(prev_tokens + total_tokens).to_be_bytes());
}

// -- Reference pricing JSON

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReferencePricing {
    pub updated_at: String,
    pub ttl_seconds: u64,
    pub mappings: Vec<PricingMapping>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PricingMapping {
    pub free_model: String,
    pub provider: String,
    pub reference_model: String,
    pub reference_provider: String,
    pub input_cost_per_1m: f64,
    pub output_cost_per_1m: f64,
    pub basis: String,
}

pub fn default_reference_pricing() -> ReferencePricing {
    ReferencePricing {
        updated_at: chrono::Utc::now().to_rfc3339(),
        ttl_seconds: 604800,
        mappings: vec![
            PricingMapping {
                free_model: "llama-3.3-70b-versatile".into(),
                provider: "groq".into(),
                reference_model: "gpt-4o-mini".into(),
                reference_provider: "openai".into(),
                input_cost_per_1m: 0.15,
                output_cost_per_1m: 0.60,
                basis: "quality-equivalent".into(),
            },
            PricingMapping {
                free_model: "llama3.1-8b".into(),
                provider: "cerebras".into(),
                reference_model: "gpt-4o-mini".into(),
                reference_provider: "openai".into(),
                input_cost_per_1m: 0.15,
                output_cost_per_1m: 0.60,
                basis: "quality-equivalent".into(),
            },
            PricingMapping {
                free_model: "Meta-Llama-3.3-70B-Instruct".into(),
                provider: "sambanova".into(),
                reference_model: "gpt-4o-mini".into(),
                reference_provider: "openai".into(),
                input_cost_per_1m: 0.15,
                output_cost_per_1m: 0.60,
                basis: "quality-equivalent".into(),
            },
            PricingMapping {
                free_model: "meta-llama/llama-3.3-70b-instruct:free".into(),
                provider: "openrouter".into(),
                reference_model: "gpt-4o-mini".into(),
                reference_provider: "openai".into(),
                input_cost_per_1m: 0.15,
                output_cost_per_1m: 0.60,
                basis: "quality-equivalent".into(),
            },
            PricingMapping {
                free_model: "mistral-small-latest".into(),
                provider: "mistral".into(),
                reference_model: "gpt-4o-mini".into(),
                reference_provider: "openai".into(),
                input_cost_per_1m: 0.15,
                output_cost_per_1m: 0.60,
                basis: "quality-equivalent".into(),
            },
        ],
    }
}

pub fn load_reference_pricing() -> ReferencePricing {
    let path = reference_pricing_path();
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                if let Ok(pricing) = serde_json::from_str(&content) {
                    return pricing;
                }
            }
            Err(_) => {}
        }
    }
    default_reference_pricing()
}

pub fn ensure_reference_pricing() {
    let path = reference_pricing_path();
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let pricing = default_reference_pricing();
    match serde_json::to_string_pretty(&pricing) {
        Ok(json) => {
            if std::fs::write(&path, json).is_ok() {
                tracing::info!(path = %path.display(), "reference_pricing.json cree");
            }
        }
        Err(e) => warn!("Erreur creation reference_pricing.json : {}", e),
    }
}

// -- Generation routing.json

#[derive(Debug, Serialize)]
pub struct RoutingJson {
    pub updated_at: String,
    pub providers: Vec<ProviderStatus>,
    pub savings: SavingsSummary,
}

#[derive(Debug, Serialize)]
pub struct ProviderStatus {
    pub name: String,
    pub status: String,
    pub quota_rpd_remaining: Option<u64>,
    pub quota_rpm_remaining: Option<u64>,
    pub quota_tpd_remaining: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct SavingsSummary {
    pub total_tokens: u64,
    pub usd_equivalent: String,
    pub reference_model: String,
}

pub fn generate_routing_json(db: &sled::Db, active_providers: &[String]) -> RoutingJson {
    let providers = active_providers.iter().map(|name| {
        let rpd = read_u64(db, &quota_rpd_key(name));
        let rpm = read_u64(db, &quota_rpm_key(name));
        let tpd = read_u64(db, &quota_tpd_key(name));

        let status = match rpd {
            Some(v) if v == 0 => "saturated".to_string(),
            Some(v) if v < 100 => "limited".to_string(),
            _ => "active".to_string(),
        };

        ProviderStatus {
            name: name.clone(),
            status,
            quota_rpd_remaining: rpd,
            quota_rpm_remaining: rpm,
            quota_tpd_remaining: tpd,
        }
    }).collect();

    let total_micro = read_u64(db, savings_total_key()).unwrap_or(0);
    let total_tokens = read_u64(db, savings_tokens_key()).unwrap_or(0);
    let usd = total_micro as f64 / 1_000_000.0;

    RoutingJson {
        updated_at: chrono::Utc::now().to_rfc3339(),
        providers,
        savings: SavingsSummary {
            total_tokens,
            usd_equivalent: format!("{:.4}", usd),
            reference_model: "gpt-4o-mini".into(),
        },
    }
}

pub fn write_routing_json(db: &sled::Db, active_providers: &[String]) {
    let routing = generate_routing_json(db, active_providers);
    let path = routing_json_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(&routing) {
        Ok(json) => {
            let _ = std::fs::write(&path, json);
        }
        Err(e) => warn!("Erreur ecriture routing.json : {}", e),
    }
}
