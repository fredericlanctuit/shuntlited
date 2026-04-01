# ShuntLited

<img width="1440" height="720" alt="image" 
src="https://github.com/user-attachments/assets/fe3bf15e-dae6-4a95-abc1-16a0039bbfde" />

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
                                      ↓  (async, session-bound)
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
It writes a JSONL file. The router reads it at startup.
Neither module knows about the other's internals.

---

## Module 1 — Frugal Router

An HTTP proxy listening on `localhost:8080` with an OpenAI-compatible API
(`POST /v1/chat/completions`). Routes each request to the best available
free provider based on live quota tracking and health checks.

ShuntLited is conditioned on API flows, not on any specific chat client.
Any client that accepts a configurable endpoint works — desktop app, mobile,
CLI, or web interface pointing to `http://localhost:8080`.

**Provider rotation example (BUILD slot):**
Cerebras → Groq → SambaNova → OpenRouter

**No hardcoded model lists.** Provider catalogues are fetched and cached from
`/v1/models` endpoints (TTL: 12 h). The LiteLLM community provider list is used
as a reference source and cached locally.

**Quota tracking** via `x-ratelimit-*` response headers and local counters
stored in `sled`. If the primary provider returns 429 or fails, the router
immediately tries the next one. No user-visible error.

**Savings counter** — every request logs tokens consumed and calculates
the equivalent cost on a reference paid provider (GPT-4o pricing).
The client displays cumulative savings since install.

**Four universal slots** define intent, not a specific model:

| Slot | Intent |
|------|--------|
| `gpt-build` | Code generation, structured output |
| `gpt-plan` | Reasoning, planning, long context |
| `gpt-chat` | Conversational, low latency |
| `gpt-distill` | Distillation calls (lightweight model) |

Slot selection is client-driven via the `model` field in the request payload.
Slot → model → provider mapping is defined in `config.toml`.

---

## Module 2 — Prompt Shield

Detects sensitive entities **locally** before any request leaves the machine.
Uses token substitution — not encryption.
The substitution map lives in memory for the duration of the request only.
Nothing is persisted.

**Example:**
```
Input:  "Mon client Dupont doit 50 000 € à Martin"
Sent:   "Mon client [PERSON_1] doit [MONTANT_1] à [PERSON_2]"
Received: "... [PERSON_1] devra régler [MONTANT_1] ..."
Output: "... Dupont devra régler 50 000 € ..."
```

### Safe to mask — v0.3 scope (whitelist)

| Entity type | Why safe |
|-------------|----------|
| Person names (first + last) | No syntactic constraints; neutral to LLM reasoning |
| Email addresses | Fixed format; placeholder preserves length |
| Phone numbers | Regex-stable; isolated from logic |
| IBAN / account numbers | Never used in reasoning chains |
| Postal addresses | Low semantic weight |

### Mask with caution — later versions

| Entity type | Risk |
|-------------|------|
| Dates (`15 mai 2024`) | LLM may do temporal reasoning; masking breaks it |
| Monetary amounts | Some models compute; masking blocks arithmetic |
| Company names | Model may rely on brand-specific knowledge |
| Contract references | Often used as logical keys; masking renders response useless |

**Implementation note:** Gender-aware placeholders (`[HOMME_NOM_1]`, `[FEMME_NOM_2]`)
are planned to preserve grammatical agreement in French-language prompts.

---

## Module 3 — Context Distiller

Reads local conversation exports (JSON from compatible chat interfaces),
sends an extraction request to the `gpt-distill` slot, and generates
a structured context package.

**This is faithful extraction, not summarisation.**
The user's exact formulations are preserved. The model extracts structure,
it does not paraphrase or compress. Information loss is a design failure here.
```json
{
  "RULES":     ["invariants the user has established"],
  "STATE":     "current project state — verbatim where possible",
  "DECISIONS": [{ "ts": "2024-09-26", "choice": "...", "rationale": "..." }],
  "NEXT":      ["planned for next session"]
}
```

Stored as `~/.shuntlited/context/latest.json` and injected
as the first system message at session start.

**This is structured semantic memory, not RAG vector retrieval.**
No embeddings. No similarity search. Simple key-based lookup.

### Distillation provider — configurable, privacy-conditioned

The distillation provider is chosen at setup and determines your privacy posture:

| Provider | Jurisdiction | GDPR posture | Recommended for |
|----------|-------------|--------------|-----------------|
| Ollama (local) | Your machine | Maximum confidentiality | All sensitive use |
| Mistral AI | France / EU | Strong — EU law applies | Professional use |
| Google / Groq / Cerebras | USA | Cloud Act applies | Personal use only |

At first launch, ShuntLited asks which profile applies to your use
and restricts distillation provider options accordingly.
See `PRIVACY.md` for full details.

### Distillation cadence

- Triggered at session close (SIGINT or inactivity timeout: 5 min)
- Overridable via `POST /distill?force=1`
- Configurable: `SHUNT_DISTILL_INTERVAL=15` (messages)

### Recommended local models for distillation

| Hardware | Model | RAM usage | Quality |
|----------|-------|-----------|---------|
| VPS 4 GB | `phi3:mini` or `gemma2:2b` | ~1.5–2.2 GB | Good for structured extraction |
| Laptop 8 GB | `mistral:7b-instruct-q4` | ~4.1 GB | Strong |
| Laptop 16 GB+ | `llama3.1:8b` | ~6 GB | Excellent |

Latency is acceptable because distillation runs async, off the critical path.

---

## Storage

**Default: zero-dependency local storage.**
```
~/.shuntlited/
  context/
    latest.json          ← injected at session start
    archive/
      2024-09-26T14-32.json
      2024-09-25T09-15.json
  state/
    quota.sled           ← provider quota counters (sled KV)
    savings.sled         ← cumulative API savings counter
```

Quota counters and provider state use `sled`
(embedded KV, ~1 MiB RAM, ACID writes, zero external dependencies).

**Optional: LanceDB for heavy users.**

Activated explicitly in config. Enables semantic search across archived
context packages — useful for users with large session histories
who need to retrieve past decisions or project states by query.
```toml
[storage]
archive = "lancedb"   # default: "jsonl"
```

The `ContextStore` trait abstracts storage from day one.
Switching from JSONL to LanceDB requires zero code changes outside the storage module.

---

## Configuration
```toml
[router]
listen = "127.0.0.1:8080"

[[providers]]
name       = "groq"
endpoint   = "https://api.groq.com/openai/v1"
api_key    = "${GROQ_API_KEY}"
slots      = ["gpt-chat", "gpt-build"]
rate_limit = 50

[[providers]]
name       = "cerebras"
endpoint   = "https://api.cerebras.ai/v1"
api_key    = "${CEREBRAS_API_KEY}"
slots      = ["gpt-build"]
rate_limit = 30

[[providers]]
name       = "mistral"
endpoint   = "https://api.mistral.ai/v1"
api_key    = "${MISTRAL_API_KEY}"
slots      = ["gpt-distill", "gpt-plan"]
rate_limit = 20

[shield]
enabled  = true
entities = ["person", "email", "phone", "iban"]

[distill]
enabled   = true
path      = "~/.shuntlited/context"
interval  = 15
slot      = "gpt-distill"
provider  = "mistral"          # or "local" for Ollama

[storage]
archive   = "jsonl"            # or "lancedb" for heavy users

[privacy]
mode      = "personal"         # or "enterprise-local"
```

### secrets.env.example
```env
# Copy this file to secrets.env and fill in your keys.
# Never commit secrets.env to version control.

# Groq — https://console.groq.com/keys
GROQ_API_KEY=

# Cerebras — https://cloud.cerebras.ai
CEREBRAS_API_KEY=

# SambaNova — https://cloud.sambanova.ai
SAMBANOVA_API_KEY=

# Mistral — https://console.mistral.ai/api-keys
MISTRAL_API_KEY=

# OpenRouter — https://openrouter.ai/settings/keys
OPENROUTER_API_KEY=
```

Provider setup guides are available in `docs/providers/`.

---

## Philosophy — FROG

**F**rugal · **R**esilient · **O**pen source · **G**ratuit

- Free tier first. Paid = explicit user choice.
- One binary. No runtime dependencies. No Docker required.
- Deployable on a 4 GB RAM VPS or any laptop.
- Open format for context packages (plain JSON) — you own your memory.
- Compatible with /e/OS and Murena environments (EU-hosted, degoogled).

---

## Technology stack

| Layer | Technology | Rationale |
|-------|-----------|-----------|
| Runtime | Rust + Tokio | Single binary, zero-runtime-deps |
| HTTP server | Axum | Middleware-native, async, layer-based |
| HTTP client | reqwest | Async, TLS built-in |
| Config | TOML + env vars | Human-readable, no secrets in files |
| Context storage | JSONL files | Zero deps, git/rsync-friendly |
| Quota state | sled | Embedded KV, ~1 MiB RAM, ACID |
| Archive (optional) | LanceDB | Semantic search for heavy users |
| Logging | tracing + daily rolling file | Structured, lightweight |

---

## Roadmap

| Version | Scope |
|---------|-------|
| **v0.1** | HTTP proxy, single provider, TOML config, logs, `/ready` health check |
| **v0.2** | Multi-provider rotation, quota tracking via `x-ratelimit-*`, savings counter |
| **v0.3** | Prompt Shield — PII whitelist (person, email, phone, IBAN) |
| **v0.4** | Context Distiller — session distillation, JSONL storage, provider choice at setup |
| **v0.5** | CLI: `shuntlited status` — quotas, active provider, recent errors, savings |
| **v0.6** | Browser extension (satellite project) — export from web interfaces to ShuntLited |
| **v1.0** | Stable config format, `/metrics` Prometheus endpoint, enterprise-local mode |
| **post-1.0** | Hollama fork client, LanceDB optional archive, /e/OS packaging |

---

## Client compatibility

ShuntLited exposes a standard OpenAI-compatible endpoint.
Any chat client that accepts a configurable API endpoint works out of the box.

Tested with: Kiro (development), Open WebUI, any OpenAI-compatible CLI.
Planned: Hollama fork with integrated savings display and project management.

---

## Known limitations

- Provider catalogues change; rotation list accuracy requires ongoing maintenance
  (LiteLLM community list used as upstream reference).
- Token substitution may degrade LLM reasoning for complex entities.
  Empirical testing per use case is required.
- Single-user only. Multi-user deployment requires enterprise-local mode
  and additional isolation — see `PRIVACY.md`.
- API keys are managed manually via `secrets.env`.
- No mobile app. A PWA served by the local binary covers mobile secondary use.
- Browser extension for importing sessions from web LLM interfaces
  is a satellite project planned for v0.6+.

---

## Privacy

See `PRIVACY.md` for the full data matrix, GDPR exposure scenarios,
usage classification by profile, and the enterprise-local configuration.

**Short version:**
- Prompt Shield processes everything locally — nothing leaves your machine.
- The router sends your (optionally masked) prompt to your configured provider.
- The distiller sends a session summary to your configured distillation provider.
- Choose a local model or Mistral EU for distillation if privacy matters to you.

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
les montants et les adresses de tes clients alimentent les données d'entraînement
d'un fournisseur américain.

**ShuntLited détecte localement les informations sensibles dans tes prompts
et les remplace par des marqueurs neutres avant l'envoi.**
La réponse revient avec les vraies valeurs réinjectées.
Aucune donnée personnelle ne quitte ta machine.

Conforme RGPD par architecture — pas par promesse de confidentialité.
Consulte `PRIVACY.md` pour les détails et les limites.

---

### L'IA se souvient de toi d'une session à l'autre.

Tu reprends un projet après deux jours. L'IA ne sait plus rien de ce que vous avez fait.
Tu perds 10 minutes à tout réexpliquer.

**ShuntLited distille l'historique de tes conversations en un paquet de contexte structuré
— tes règles, l'état du projet, les décisions prises, la prochaine étape —
et l'injecte automatiquement au démarrage de chaque nouvelle session.**

Ce n'est pas de la recherche vectorielle. C'est de la mémoire structurée, lisible, portable.
Ton contexte est un fichier JSON. Tu le possèdes.
Les mots que tu as utilisés restent les tiens — ShuntLited extrait, il ne reformule pas.

---

### ShuntLited te montre ce que tu économises.

Chaque requête routée vers un fournisseur gratuit est comparée
au tarif équivalent sur une API payante de référence.
Un compteur visible dans le client affiche :

- Crédits économisés aujourd'hui
- Crédits économisés ce mois
- Total depuis l'installation

Tu vois la valeur de ShuntLited en temps réel.

---

### Trois profils, un seul outil

**L'étudiant** — pas de carte bancaire, quotas épuisés avant la fin du devoir.
ShuntLited tourne en fond, redistribue entre Groq, Cerebras, SambaNova.
La limite disparaît de ta vue.

**La personne surchargée** — 40 messages ChatGPT partis en réunion,
et il reste encore la moitié de la journée.
ShuntLited bascule vers un autre fournisseur sans changer d'interface.

**Le freelance RGPD** — les noms de tes clients ne voyagent pas en clair.
Tu peux utiliser l'IA pour ton travail sans compromettre leur confidentialité.

**Le power user** — 10 fils de réflexion en parallèle, des décisions à retrouver,
des projets qui s'étendent sur des semaines. ShuntLited garde le fil.
Tu reprends là où tu t'es arrêté, avec le contexte intact.

---

*Un seul binaire. Aucune dépendance. Tourne sur un VPS à 4 Go ou sur ton laptop.*
*Gratuit d'abord. Payant = ton choix explicite.*
