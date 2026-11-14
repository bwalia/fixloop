# fixloop-llm

Model access for Fixloop. Everything else talks to models through `Router`, never to a vendor API directly.

- **Providers:** Anthropic, OpenAI, and any OpenAI-compatible server (presets for Ollama, vLLM, llama.cpp, LM Studio, NVIDIA NIM).
- **Routing:** per-task fallback chains from config; retries with backoff on 429/5xx/timeouts.
- **Data egress:** `local-only` repositories only ever reach self-hosted models. If no local model is routed, the call fails rather than downgrading.
- **Budgets:** per-job and daily token and cost limits, reserved before each call and settled after.
- **Tools:** native tool calling, or a JSON fallback (`tool_mode: json`) for models without it.
- **Egress guard:** each client can only reach its configured endpoint; redirects are off; offline mode rejects external APIs.
- **Tracing:** OpenTelemetry GenAI attributes on every call.

```rust
let router = Router::from_config(&config.llm, &|k| std::env::var(k).ok())?;
let ctx = CallContext::new(DataEgress::Any).with_budget(router.job_budget("job-42"));
let reply = router.complete(Task::Review, &CompletionRequest::from_prompt("…", 1024), &ctx).await?;
```

See [ADR-0004](../../docs/adr/0004-llm-provider-and-routing.md) and the [config reference](../../docs/config-reference.md#llm).

License: Apache-2.0
