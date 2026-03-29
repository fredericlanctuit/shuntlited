# ShuntLited

> **Your conversations stay yours. You pay nothing. You never start from scratch.**

A Rust binary that sits between your AI interface and cloud providers.  
It routes intelligently to free tiers, masks sensitive data before transit, and distills past sessions into reusable context.

---

## The Problem

Cloud AI is powerful but has three structural flaws nobody solves together:

- **It costs money** the moment you exceed free limits
- **It collects your data** by design — your prompts train their models
- **It forgets everything** between sessions — you restart from zero every time

Local solutions (Ollama, llama.cpp) require hardware most people don't have.  
Existing proxies (LiteLLM, OpenRouter) handle routing but not privacy or context.

---

## What ShuntLited Does
```
Your interface (Kiro)
        │
        ▼
┌───────────────────────┐
│      ShuntLited       │
│                       │
│  1. Context Distiller │  ← injects memory from past sessions
│  2. Prompt Shield     │  ← masks sensitive data before transit
│  3. Frugal Router     │  ← routes to best free-tier provider
└───────────┬───────────┘
            │
            ▼
   Cloud Providers
   (Groq, Mistral, Google, Cerebras...)
   → they never see your raw sensitive data
   → they never see your conversation history
```

---

## Three Modules

### 1. Frugal Router
Routes each request to the most appropriate free-tier provider based on available quotas and performance scores. Falls back automatically if a provider is down or quota is exhausted.

**Universal slots:**
| Slot | Purpose | Priority order |
|------|---------|----------------|
| `gpt-build` | Code, refactoring | Cerebras → Groq → SambaNova → OpenRouter |
| `gpt-plan` | Reasoning, architecture | Groq → SambaNova → OpenRouter → Cerebras |
| `gpt-chat` | Conversation, daily use | OpenRouter → Scaleway → Groq → Mistral |
| `gpt-distill` | Context distillation only | Google → Mistral → Groq |

No hardcoded model lists — the catalogue updates automatically from provider `/v1/models` endpoints.

---

### 2. Prompt Shield
Masks sensitive data before it leaves your machine using token substitution.
```
"My client Dupont owes €50,000 to Martin"
        │
        ▼  (local substitution)
"My client [ENTITY_1] owes [AMOUNT_1] to [ENTITY_2]"
        │
        ▼  (sent to provider)
"[ENTITY_1] should negotiate a payment plan with [ENTITY_2]"
        │
        ▼  (local reinjection)
"Dupont should negotiate a payment plan with Martin"
```

The provider reasons on the structure. It never sees the sensitive values.  
**GDPR-compliant by design, not by privacy policy.**

---

### 3. Context Distiller
Transforms your past conversations into reusable structured context.

Reads Kiro's local IndexedDB exports (JSON), sends them to a lightweight
distillation model (Gemini Flash Lite — 1M token context window, free tier),
and produces a `context-packet`:
```
RULES      → your invariants (never changes)
STATE      → current project or conversation state
DECISIONS  → timestamped choices with reasoning
NEXT       → what was planned for the next session
```

This packet is injected as a system prompt at the start of each new session.  
**You never explain your context again.**

> Why not RAG? A vector RAG retrieves fragments. The Context Distiller
> extracts structure — rules and decisions, not raw text.

---

## Philosophy

**FROG** — the internal compass for every technical decision:

| | Value | Concrete application |
|--|-------|---------------------|
| **F** | Frugality | Free-tier first, small model for distillation, zero runtime dependency |
| **R** | Resilience | Auto-fallback, graceful degradation, works without internet for masking |
| **O** | Open-source | Public Rust binary, open context-packet format |
| **G** | Gratis | Zero cost by default — paid providers are an explicit user choice |

---

## Who Is This For

**The teenager learning with AI**  
No credit card. No subscription. No questions ending up in a training
dataset somewhere.
> *Stop paying to learn. Your questions stay yours.*

**The busy person with too much on their mind**  
Already hit the 40-message limit on ChatGPT. Doesn't want to think
about quotas.
> *Your head is already full. No need to add a bill.*

**The freelancer handling client data**  
Needs AI in their daily workflow but can't send client data to OpenAI
without thinking twice.
> *AI in your workflow. Your client data stays with you.*

---

## Tech Stack

| Component | Choice | Why |
|-----------|--------|-----|
| Language | Rust (Edition 2021+) | Memory precision, autonomous binary, transparent proxy latency |
| Async runtime | Tokio | Concurrent requests without overhead |
| HTTP server | Axum | Minimal, fast, idiomatic Rust |
| Masking engine | Custom (Rust) | Zero external dependency, auditable |
| Serialization | Serde + JSON | Open context-packet format |
| Config | TOML | Human-readable, non-developer friendly |
| Distribution | Single binary | Zero install, zero runtime, zero `pip install` |

---

## Roadmap

| Version | Content | Status |
|---------|---------|--------|
| `v0.1` | Frugal Router — HTTP proxy, 4 slots, provider rotation | 🚧 |
| `v0.2` | Dynamic catalogue — auto-discovery from `/v1/models` | 📋 |
| `v0.3` | Context Distiller — Kiro JSON parsing, context-packet format | 📋 |
| `v0.4` | Prompt Shield — token masking, local reinjection | 📋 |
| `v1.0` | Full stack — `install.sh`, user doc, Kiro integration | 📋 |

Each version is usable standalone. v0.1 does not block v0.4.

---

## vs. Existing Tools

| | ShuntLited | LiteLLM | OpenRouter | Ollama |
|--|-----------|---------|------------|--------|
| Free-tier routing | ✅ | ✅ | ✅ | ❌ |
| Sensitive data masking | ✅ | ❌ | ❌ | N/A |
| Persistent context | ✅ | ❌ | ❌ | ❌ |
| Single binary | ✅ | ❌ | N/A | ✅ |
| No GPU required | ✅ | ✅ | ✅ | ⚠️ |
| GDPR by design | ✅ | ⚠️ | ⚠️ | ✅ |
| Non-developer friendly | ✅ | ❌ | ❌ | ❌ |

---

## Contributing

This project is in early design phase. The most valuable contributions
right now:

- **Architecture feedback** — open an issue, challenge the design
- **Rust expertise** — Tokio, Axum, NER crates, token masking
- **Real-world use cases** — what sensitive data patterns should
  Prompt Shield handle?
- **Provider knowledge** — free-tier limits, quirks, undocumented
  endpoints

No code required to contribute at this stage.  
A good issue is worth more than a rushed PR.

---

## Status

**Early design — no code yet.**  
The architecture is defined. The roadmap is set.  
Looking for early feedback and contributors.

If you're building something in this space, open an issue. Let's talk.

---

## License

MIT — do whatever you want, keep the attribution.

---

*Built on the belief that useful AI should not require a credit card
or a privacy waiver.*
