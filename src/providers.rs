// Mapping providers : meta techniques par provider
// SambaNova headers : a confirmer par curl (marques TODO)

pub struct ProviderMeta {
    pub base_url: &'static str,
    pub default_model: &'static str,
    pub header_rpd: Option<&'static str>,
    pub header_rpm: Option<&'static str>,
    pub header_tpd: Option<&'static str>,
}

const GROQ: ProviderMeta = ProviderMeta {
    base_url: "https://api.groq.com/openai/v1",
    default_model: "llama-3.3-70b-versatile",
    header_rpd: Some("x-ratelimit-remaining-requests"),
    header_rpm: None,
    header_tpd: None,
};

const CEREBRAS: ProviderMeta = ProviderMeta {
    base_url: "https://api.cerebras.ai/v1",
    default_model: "llama3.1-8b",
    header_rpd: Some("x-ratelimit-remaining-requests-day"),
    header_rpm: Some("x-ratelimit-remaining-tokens-minute"),
    header_tpd: Some("x-ratelimit-remaining-tokens-day"),
};

const SAMBANOVA: ProviderMeta = ProviderMeta {
    base_url: "https://api.sambanova.ai/v1",
    default_model: "Meta-Llama-3.3-70B-Instruct",
    header_rpd: Some("x-ratelimit-remaining-requests-day"),
    header_rpm: None,
    header_tpd: None,
};

const OPENROUTER: ProviderMeta = ProviderMeta {
    base_url: "https://openrouter.ai/api/v1",
    default_model: "meta-llama/llama-3.3-70b-instruct:free",
    header_rpd: Some("x-ratelimit-remaining"),
    header_rpm: None,
    header_tpd: None,
};

const MISTRAL: ProviderMeta = ProviderMeta {
    base_url: "https://api.mistral.ai/v1",
    default_model: "mistral-small-latest",
    header_rpd: Some("x-ratelimit-remaining-requests"),
    header_rpm: Some("x-ratelimit-remaining-tokens"),
    header_tpd: None,
};

const SCALEWAY: ProviderMeta = ProviderMeta {
    base_url: "https://api.scaleway.ai/v1",
    default_model: "llama-3.3-70b-instruct",
    header_rpd: None,
    header_rpm: None,
    header_tpd: None,
};

pub fn get_provider_meta(name: &str) -> Option<&'static ProviderMeta> {
    match name {
        "groq"       => Some(&GROQ),
        "cerebras"   => Some(&CEREBRAS),
        "sambanova"  => Some(&SAMBANOVA),
        "openrouter" => Some(&OPENROUTER),
        "mistral"    => Some(&MISTRAL),
        "scaleway"   => Some(&SCALEWAY),
        _            => None,
    }
}
