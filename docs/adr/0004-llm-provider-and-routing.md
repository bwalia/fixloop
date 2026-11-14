# ADR-0004: LLM provider abstraction, routing, fallback and budgets

* Status: Proposed
* Date: 2026-11-14

## Context
We need frontier (Anthropic, OpenAI, Gemini, Azure OpenAI, Bedrock, Vertex) and local (OpenAI-compatible: Ollama, vLLM, llama.cpp, LM Studio, NIM) models. There must be no hard-coded model names, per-task routing, fallback chains, budgets, a provable offline mode, and per-repo data-egress rules.

## Decision

### Trait
```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &ProviderId;
    fn capabilities(&self, model: &ModelId) -> ModelCaps; // tools, json_schema, vision, ctx window, pricing
    fn locality(&self) -> Locality;                        // Local | SelfHosted | External
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, LlmError>;
    async fn stream(&self, req: CompletionRequest) -> Result<BoxStream<'static, Result<Delta, LlmError>>, LlmError>;
    async fn embed(&self, req: EmbedRequest) -> Result<EmbedResponse, LlmError>;
}
```
* Providers are implemented over `reqwest` + rustls with thin per-vendor request mappers. No vendor SDKs, which keeps the dependency and license surface small. Bedrock signing uses `aws-sigv4`. Vertex uses Google ADC tokens.
* `ModelCaps` comes from **config** (with optional built-in hint tables that the config can override). Model names live only in config.

### Routing
Tasks are `triage | dedupe | embed | review | fix | second_review | explain | summarize`.
```yaml
llm:
  providers:
    claude:  { kind: anthropic, api_key_env: ANTHROPIC_API_KEY }
    local:   { kind: openai_compatible, preset: ollama, base_url: http://localhost:11434/v1 }
  models:
    strong:  { provider: claude, model: "<from config>", price: {in: ..., out: ...} }
    cheap:   { provider: local,  model: "<from config>" }
  routes:
    review:        [strong, cheap]   # ordered fallback chain
    triage:        [cheap]
    second_review: [strong]          # must differ from the `fix` model when possible
```
Resolution order: repo `.fixloop.yaml` → repo policy → team → org → global. Each candidate is filtered by:
1. **Data-egress class**: a repo set to `local-only` filters out every `External` provider. If nothing is left, it is a hard error (never a silent downgrade).
2. Capability (tools, context window large enough for the prompt).
3. Budget headroom.

### Fallback
Retry with backoff on retryable errors (429, 5xx, timeouts) up to the per-provider limit, then move to the next chain entry. A circuit breaker per provider is kept in memory. Every attempt is a span carrying GenAI semconv attributes.

### Tool calling
Native tools are used when `caps.tools`. Otherwise a **JSON-mode shim** puts the tool schemas into the prompt, parses a strict JSON envelope `{tool, args}`, validates it against the schema, and re-prompts once on parse failure.

### Budgets
Hierarchical meters (per job, per PR, per repo/day, per team/day, org/day). They reserve an estimate before each call and reconcile actual usage after it. Exceeding a budget gives `LlmError::BudgetExceeded`; the job stops gracefully and reports what it finished.

### Offline mode
`llm.offline: true` → the HTTP client factory installs a connector that refuses any host not in the configured local endpoints. Startup fails if any configured route points at an `External` provider. **Test:** run a full review with offline config, with a catch-all mock that panics on any unexpected host, and assert zero external connections.

## Consequences
We maintain request mappers ourselves (about 6 frontier APIs). This is acceptable: they are small, fully covered by recorded fixtures, and remove SDK churn.
