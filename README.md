# ShuntLited

<img width="1440" height="720" alt="image" src="https://github.com/user-attachments/assets/fe3bf15e-dae6-4a95-abc1-16a0039bbfde" />

# ShuntLited

> A single Rust binary acting as a local interceptor between your AI chat interface
> and cloud LLM providers — frugal by design, private by architecture.

---

## Why ShuntLited?

Three problems affect anyone using AI chat tools daily:

**Cost** — Free-tier quotas run out fast. Paid APIs add up. ShuntLited rotates across
free providers automatically, so you get more tokens before spending a cent.

**Privacy** — Every prompt sent to a cloud provider feeds their training pipeline.
ShuntLited masks sensitive entities locally before any data leaves your machine.

**Memory** — LLMs forget everything between sessions. ShuntLited distils your
conversation history into a structured context package injected at session start.

---

## Architecture

Three components, two execution modes:
```
Chat client → [Prompt Shield] → [Frugal Router] → Free LLM provider
                                      ↓  (async)
                             [Context Distiller]
                                      ↓
                          ~/.shuntlited/context/latest.json
                                      ↓  (injected at next session start)
                                 Chat client
```

### Critical path (synchronous, per-request)

**Prompt Shield** and **Frugal Router** live on the hot path.
Every request passes through both before reaching the provider.
Response comes back through the same pipeline.

### Async path (session-bound)

**Context Distiller** is completely decoupled from the critical path.
It runs out-of-band: triggered by session close, inactivity timeout, or manual call.
It writes a JSONL file; the router reads it at startup. Neither knows about the other's internals.

---

## Module 1 — Frugal Router

An HTTP proxy listening on `localhost:8080` with an OpenAI-compatible API
(`POST /v1/chat/completions`). Routes each request to the best available
free provider based on live quota tracking and health checks.

**Provider rotation example (BUILD slot):**
Cerebras → Groq → SambaNova → OpenRouter

**No hardcoded model lists.** Provider catalogues are fetched and cached from
`/v1/models` endpoints (TTL: 12 h). Quota state is tracked via `x-ratelimit-*`
response headers and local counters.

**Fallback:** if the primary provider returns 429 or fails, the router
immediately tries the next one in the rotation. No user-visible error.

**Four universal slots** define intent, not a specific model:

| Slot | Intent |
|------|--------|
| `gpt-build` | Code generation, structured output |
| `gpt-plan` | Reasoning, planning, long context |
| `gpt-chat` | Conversational, low latency |
| `gpt-distill` | Distillation calls (lightweight model) |

Slot selection is client-driven via the `model` field in the request payload.

---

## Module 2 — Prompt Shield

Detects sensitive entities **locally** before any request leaves the machine.
Uses token substitution — not encryption. RGPD-compliant by design, not by policy.

**Example:**
```
Input:  "Mon client Dupont doit 50 000 € à Martin"
Sent:   "Mon client [PERSON_1] doit [MONTANT_1] à [PERSON_2]"
Received: "... [PERSON_1] devra régler [MONTANT_1] ..."
Output: "... Dupont devra régler 50 000 € ..."
```

The substitution map lives in memory for the duration of the request. Nothing is persisted.

### Safe to mask (whitelist — v0.2 scope)

| Entity type | Why safe |
|-------------|----------|
| Person names (first + last) | No syntactic constraints; neutral to LLM reasoning |
| Email addresses | Fixed format; placeholder preserves length |
| Phone numbers | Regex-stable; isolated from logic |
| IBAN / account numbers | Never used in reasoning chains |
| Postal addresses | Low semantic weight |

### Mask with caution (later versions)

| Entity type | Risk |
|-------------|------|
| Dates (`15 mai 2024`) | LLM may do temporal reasoning ("in 3 days"); masking breaks it |
| Monetary amounts | Some models compute; masking blocks arithmetic |
| Company names | Model may have brand-specific knowledge it relies on |
| Contract references | Often used as logical keys; masking makes response useless |

**Implementation note:** Gender-aware placeholders (`[HOMME_NOM_1]`, `[FEMME_NOM_2]`)
are planned to preserve grammatical agreement in French-language prompts.

---

## Module 3 — Context Distiller

Reads local conversation exports (JSON from Kiro or compatible chat interfaces),
sends a summary request to the `gpt-distill` slot (Gemini Flash Lite or equivalent),
and generates a structured context package:
```json
{
  "RULES":     ["invariants the user has established"],
  "STATE":     "current project state summary",
  "DECISIONS": [{ "ts": "2024-09-26", "choice": "...", "rationale": "..." }],
  "NEXT":      ["planned for next session"]
}
```

This package is stored as `~/.shuntlited/context/latest.json` and injected
as the first system message at session start.

**This is structured semantic memory, not RAG vector retrieval.**
No embeddings. No similarity search. Simple key-based lookup.

### Distillation cadence (v0.1 default)

- Triggered at session close (SIGINT or inactivity timeout: 5 min)
- Overridable via `POST /distill?force=1`
- Configurable interval: `SHUNT_DISTILL_INTERVAL=15` (messages)

---

## Storage

**No database for context packages.** LanceDB was evaluated and rejected as
over-engineered for this workload (a few dozen KB of JSON per day, key-based access only).

Context packages are stored as JSONL files:
```
~/.shuntlited/context/
  latest.json          ← injected at session start
  archive/
    2024-09-26T14-32.json
    2024-09-25T09-15.json
```

Quota counters and provider state use `sled` (embedded key-value, ~1 MiB RAM,
ACID writes, zero external dependencies).

---

## Configuration
```toml
[router]
listen = "127.0.0.1:8080"

[[providers]]
name     = "groq"
endpoint = "https://api.groq.com/openai/v1"
api_key  = "${GROQ_API_KEY}"
slots    = ["gpt-chat", "gpt-build"]
rate_limit = 50

[[providers]]
name     = "cerebras"
endpoint = "https://api.cerebras.ai/v1"
api_key  = "${CEREBRAS_API_KEY}"
slots    = ["gpt-build"]
rate_limit = 30

[shield]
enabled  = true
entities = ["person", "email", "phone", "iban"]

[distill]
enabled  = true
path     = "~/.shuntlited/context"
interval = 15
slot     = "gpt-distill"
```

---

## Philosophy — FROG

**F**rugal · **R**esilient · **O**pen source · **G**ratuit

- Free tier first. Paid = explicit user choice.
- One binary. No runtime dependencies. No Docker required.
- Deployable on a 4 GB RAM VPS or any laptop.
- Open format for context packages (plain JSON) — you own your memory.

---

## Technology stack

| Layer | Technology | Rationale |
|-------|-----------|-----------|
| Runtime | Rust + Tokio | Single binary, zero-runtime-deps |
| HTTP server | Axum | Middleware-native, async |
| HTTP client | reqwest | Async, TLS built-in |
| Config | TOML + env vars | Human-readable, no secrets in files |
| Context storage | JSONL files | Zero deps, git/rsync-friendly |
| Quota state | sled | Embedded KV, 1 MiB RAM, ACID |
| Logging | tracing + daily rolling file | Structured, lightweight |

---

## Roadmap

| Version | Scope |
|---------|-------|
| **v0.1** | HTTP proxy + single provider + TOML config + logs + `/ready` health check |
| **v0.2** | Frugal router: multi-provider rotation + quota tracking via `x-ratelimit-*` |
| **v0.3** | Prompt Shield: PII whitelist (person, email, phone, IBAN) |
| **v0.4** | Context Distiller: session-based distillation + JSONL storage |
| **v0.5** | CLI: `shuntlited status` showing quotas, active provider, recent errors |
| **v1.0** | Stable config format + `/metrics` endpoint (Prometheus) |

---

## Known limitations

- Provider catalogues change frequently; keeping the rotation list accurate requires maintenance.
- Token substitution can degrade LLM reasoning for complex entities (dates, amounts). Empirical testing per use case is required.
- No multi-user support. Designed for single-user, local deployment.
- API keys must be managed manually in environment variables or config.

---

## Status

Phase: **early design + Cargo scaffold**
`cargo build` compiles. No functional HTTP server yet.
Contributions and issue reports welcome.

---

## License

MIT

---

*ShuntLited is not affiliated with any LLM provider.*

## ## ## ## ## ## ## ## ## ## ## ## ## ## ## ## ## ## ## ## ## ## ## ## 

---

## ShuntLited — Pour qui, pourquoi

### L'IA gratuite a une limite. ShuntLited la repousse.

Tu utilises ChatGPT, Claude ou Gemini tous les jours.  
À un moment tu tombes sur ce message : *"Vous avez atteint votre limite de messages."*  
Et tu n'as pas de carte bancaire — ou tu ne veux pas payer.

**ShuntLited intercepte tes requêtes avant qu'elles arrivent au cloud,  
les redirige automatiquement vers le meilleur fournisseur gratuit disponible,  
et recommence quand le quota est épuisé.**

Zéro configuration complexe. Lance le binaire, pointe ton client IA dessus, c'est tout.

---

### Tes données restent chez toi.

Tu es freelance. Tu gères des dossiers clients. Tu ne veux pas que les noms,
les montants et les adresses de tes clients alimentent les données d'entraînement d'un fournisseur américain.

**ShuntLited détecte localement les informations sensibles dans tes prompts
et les remplace par des marqueurs neutres avant l'envoi.**  
La réponse revient avec les vraies valeurs réinjectées.  
Aucune donnée personnelle ne quitte ta machine.

Conforme RGPD par architecture — pas par promesse de confidentialité.

---

### L'IA se souvient de toi d'une session à l'autre.

Tu reprends un projet après deux jours. L'IA ne sait plus rien de ce que vous avez fait.  
Tu perds 10 minutes à tout réexpliquer.

**ShuntLited distille l'historique de tes conversations en un paquet de contexte structuré
— tes règles, l'état du projet, les décisions prises, la prochaine étape —
et l'injecte automatiquement au démarrage de chaque nouvelle session.**

Ce n'est pas de la recherche vectorielle. C'est de la mémoire structurée, lisible, portable.  
Ton contexte est un fichier JSON. Tu le possèdes.

---

### Trois profils, un seul outil

**L'ado qui apprend** — pas de carte bancaire, quotas épuisés avant la fin du devoir.  
ShuntLited tourne silencieusement en fond, redistribue entre Groq, Cerebras, SambaNova.  
La limite disparaît de ta vue.

**La personne surchargée** — 40 messages ChatGPT partis en réunion de travail,  
et il reste encore la moitié de la journée.  
ShuntLited bascule vers un autre fournisseur sans que tu aies à changer d'interface.

**Le freelance soucieux du RGPD** — les noms de tes clients ne voyagent pas en clair.  
Tu peux utiliser l'IA pour ton travail sans compromettre leur confidentialité.

---

*Un seul binaire. Aucune dépendance. Tourne sur un VPS à 4 Go ou sur ton laptop.*  
*Gratuit d'abord. Payant = ton choix explicite.*
