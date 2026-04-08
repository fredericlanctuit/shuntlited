![ShuntLited](https://img.shields.io/badge/ShuntLited-v0.2-black?style=flat-square)
![Rust](https://img.shields.io/badge/Rust-1.94-orange?style=flat-square&logo=rust)
![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)
![FROG](https://img.shields.io/badge/FROG-Frugal%20%7C%20Resilient%20%7C%20Open%20%7C%20Free-4a9?style=flat-square)
![Free tier](https://img.shields.io/badge/Free%20tier-first-blue?style=flat-square)

# ShuntLited

<img width="1440" height="720" alt="image" 
src="https://github.com/user-attachments/assets/fe3bf15e-dae6-4a95-abc1-16a0039bbfde" />

> A single Rust binary sitting between your AI chat client and cloud LLM providers.
> Frugal by design. Private by architecture. Zero runtime dependencies.

---

## Why ShuntLited?

Three problems affect anyone using AI tools daily:

**Cost** — Free-tier quotas run out fast. ShuntLited rotates across free providers
automatically, tracking quotas in real time. You get more tokens before spending a cent.
A savings counter shows you exactly what you would have paid on a reference paid API.

**Privacy** — Every prompt sent to a cloud provider may feed their training pipeline.
ShuntLited masks sensitive entities locally before anything leaves your machine,
using token substitution — not encryption. The provider reasons on structure, not values.

**Memory** — LLMs forget everything between sessions. ShuntLited distils conversation
history into a structured context package injected at session start.
Not RAG. Not vectors. Structured, readable, portable JSON you own.

---

## Architecture

Two execution paths, zero coupling between modules:

```
Chat client → [IPS: Prompt Shield] → [IFR: Frugal Router] → LLM provider
                                              ↓ (async, session-bound)
                                     [ICB: Context Brain]
                                              ↓
                              ~/.shuntlited/projects/<uuid>.json
                                              ↓ (injected at next session start)
                                         Chat client
```

**Critical path (synchronous, per-request):**
IPS + IFR on every request. Latency target: transparent.

**Async path (session-bound):**
ICB runs out-of-band. Neither module knows the other's internals.

---

## Three modules

### IFR — Intelligent Frugal Router

HTTP proxy on `localhost:8080`, OpenAI-compatible API (`POST /v1/chat/completions`).
Routes each request to the best available free provider based on live scoring.

**Four universal slots** — intent-based, not model-specific:

| Slot | Intent |
|------|--------|
| `gpt-build` | Code, structured output |
| `gpt-plan` | Reasoning, long context |
| `gpt-chat` | Conversational, low latency |
| `gpt-distill` | Distillation (privacy-conditioned provider) |

**Supported free-tier providers:** Groq, Cerebras, SambaNova, OpenRouter, Mistral, Scaleway

**Routing config (`config.toml`):**
```toml
[slots.gpt-build]
providers = ["cerebras", "groq", "sambanova", "openrouter"]

[slots.gpt-plan]
providers = ["groq", "sambanova", "openrouter", "scaleway"]

[slots.gpt-chat]
providers = ["openrouter", "scaleway", "groq", "cerebras"]

[slots.gpt-distill]
providers = ["mistral", "groq"]
```

At startup, ShuntLited filters each slot to providers with a key present in `secrets.env`.
A missing key is silently ignored — no error, no crash.

**Scoring (per-request, algorithmic):**
```
Score = 0.4 × quota_score + 0.2 × latency_score + 0.3 × quality_score + 0.1 × freshness_score
```

On 429: cooldown stored in sled (`max(retry-after, 300s)`), next provider tried immediately.
If all providers are exhausted: actionable error with estimated wait time.

**Savings counter:** each request calculates equivalent cost on a reference paid model.
Stored in `savings.sled`. Displayed by the client in real time.

---

### IPS — Intelligent Prompt Shield *(v0.3)*

Local middleware. Nothing leaves the machine during masking.
Token substitution — the provider receives structure, not values.
Substitution map lives in memory for the duration of the request only.

**v0.3 scope (whitelist):** person names, emails, phone numbers, IBAN, postal addresses.

Gender-aware placeholders for French: `[HOMME_NOM_1]`, `[FEMME_NOM_2]`.

Entities out of scope in v0.3 (documented risk of LLM reasoning degradation):
dates, monetary amounts, company names, contract references.

Bypass per-request: `X-Shunt-Shield: off` header.

---

### ICB — Intelligent Context Brain *(v0.4)*

Structured persistent memory. Not a session summariser.
Distils conversation history into a cumulative context package, session after session.

**Configurable fidelity and search levels (0-5):**

| Level | Description | RAM |
|-------|-------------|-----|
| 0 | No distillation | 0 |
| 1 | Structured JSON per session, 1 LLM call | ~1 MB |
| 2 | Cumulative JSON, merged across sessions | ~2 MB |
| 3 | Progressive distillation every N messages (default N=10) | ~5 MB |
| 4 | Level 3 + lightweight embeddings (semantic search) | ~80-200 MB |
| 5 | Full RAG + LanceDB (laptop ≥16 GB recommended) | ~4 GB |

Default: fidelity=2, search=2. Levels 1-3 invisible on a 4 GB VPS.

**Distillation provider — privacy-conditioned, no silent fallback:**
1. `OLLAMA_BASE_URL` present → local, maximum confidentiality
2. `MISTRAL_API_KEY` present → Mistral EU, GDPR-friendly
3. Neither → ICB disabled, warning at startup

---

## Secrets

```bash
cp secrets.env.example secrets.env
# Fill in the keys you have. Missing keys = provider ignored.
```

```env
GROQ_API_KEY=
CEREBRAS_API_KEY=
SAMBANOVA_API_KEY=
OPENROUTER_API_KEY=
MISTRAL_API_KEY=
SCALEWAY_API_KEY=
```

All providers are optional. ShuntLited starts with whatever keys are present.

---

## Quick start

```bash
git clone https://github.com/fredericlanctuit/shuntlited
cd shuntlited
cp secrets.env.example secrets.env   # add your keys
cargo build --release
./target/release/shuntlited
```

Point any OpenAI-compatible client to `http://localhost:8080`.
Use `gpt-chat`, `gpt-build`, `gpt-plan` or `gpt-distill` as the model name.

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-chat","messages":[{"role":"user","content":"Hello"}]}'
```

---

## Storage

```
~/.shuntlited/
  data/
    catalogue.json        models + pricing (TTL 12h)
    scoring.json          scores per slot (TTL 12h)
    routing.json          live quotas + savings
    reference_pricing.json  reference paid model pricing (TTL 7d)
  projects/
    <uuid>.json           project metadata + distilled context
  state/
    quota.sled            quota counters per provider
    savings.sled          cumulative savings counter
```

---

## Roadmap

| Version | Scope | Status |
|---------|-------|--------|
| v0.1 | HTTP proxy, single provider, TOML config, `/ready` | ✅ done |
| v0.2 | Multi-provider rotation, 429 fallback + cooldown sled, savings structure | ✅ done |
| v0.3 | Catalogue Manager (Rust), Project Manager, IPS Prompt Shield, `/data/*` endpoints | 🔜 next |
| v0.4 | ICB — dedicated distillation prompt session first, then code | planned |
| v0.5 | CLI: `shuntlited status` | planned |
| v0.6 | Browser extension (satellite) | planned |
| v1.0 | KiLolla client integrated, stable config, `/metrics` Prometheus | planned |
| post-1.0 | LanceDB optional, /e/OS packaging, Murena Cloud storage | vision |

---

## Client compatibility

ShuntLited exposes a standard OpenAI-compatible endpoint.
Any client with a configurable endpoint works: desktop apps, CLIs, web UIs.

**KiLolla** — the official client, currently in design.
HTML/JS vanilla, zero build dependencies, served by the ShuntLited binary itself.
Development starts after v0.3 (`/data/*` endpoints available).

---

## Philosophy — FROG

**F**rugal · **R**esilient · **O**pen source · **G**ratuit

- Free tier first. Paid = explicit user choice.
- One binary. No runtime dependencies. No Docker.
- Deployable on a 4 GB VPS or any laptop.
- Plain JSON context packages — you own your memory.
- Compatible with /e/OS and Murena environments.

---

## Stack

| Layer | Technology |
|-------|-----------|
| Runtime | Rust + Tokio |
| HTTP server | Axum |
| HTTP client | reqwest |
| Config | TOML + secrets.env |
| Context storage | JSONL |
| Quota state | sled (embedded KV) |
| Archive (optional) | LanceDB |

---

## Status

**v0.2 — multi-provider rotation operational.**
5 providers active (Groq, Cerebras, SambaNova, OpenRouter, Mistral).
429 fallback with sled cooldown tested and validated.
IPS and ICB in design — v0.3 next.

---

## License

MIT — contributions welcome. Read `docs/04_decisions.md` before opening a PR.

---

*ShuntLited is not affiliated with any LLM provider.*

---

## Pour qui, pourquoi

### L'IA gratuite a une limite. ShuntLited la repousse.

Tu utilises ChatGPT, Claude ou Gemini tous les jours.
A un moment tu tombes sur ce message : *"Vous avez atteint votre limite de messages."*
Et tu n'as pas de carte bancaire — ou tu ne veux pas payer.

**ShuntLited intercepte tes requetes avant qu'elles arrivent au cloud,
les redirige automatiquement vers le meilleur fournisseur gratuit disponible,
et recommence quand le quota est epuise.**

Zero configuration complexe. Lance le binaire, pointe ton client IA dessus, c'est tout.

---

### Tes donnees restent chez toi.

Tu es freelance. Tu geres des dossiers clients. Tu ne veux pas que les noms,
les montants et les adresses de tes clients alimentent les donnees d'entrainement
d'un fournisseur americain.

**ShuntLited detecte localement les informations sensibles dans tes prompts
et les remplace par des marqueurs neutres avant l'envoi.**
La reponse revient avec les vraies valeurs reinjectees.
Aucune donnee personnelle ne quitte ta machine.

---

### L'IA se souvient de toi d'une session a l'autre.

Tu reprends un projet apres deux jours. L'IA ne sait plus rien de ce que vous avez fait.
Tu perds 10 minutes a tout reexpliquer.

**ShuntLited distille l'historique de tes conversations en un paquet de contexte structure
et l'injecte automatiquement au demarrage de chaque nouvelle session.**

Ce n'est pas de la recherche vectorielle. C'est de la memoire structuree, lisible, portable.
Ton contexte est un fichier JSON. Tu le possedes.

---

### ShuntLited te montre ce que tu economises.

Chaque requete routee vers un fournisseur gratuit est comparee
au tarif equivalent sur une API payante de reference.

---

*Un seul binaire. Aucune dependance. Tourne sur un VPS a 4 Go ou sur ton laptop.*
*Gratuit d'abord. Payant = ton choix explicite.*
