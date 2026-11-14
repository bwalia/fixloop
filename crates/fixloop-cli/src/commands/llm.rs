//! `fixloop llm`: inspect model routing and test models.

use std::time::Instant;

use clap::{Args, Subcommand};
use fixloop_core::FixloopConfig;
use fixloop_core::Hinted;
use fixloop_core::config::Task;
use fixloop_core::domain::DataEgress;
use fixloop_llm::{CallContext, CompletionRequest, LlmError, Router, ToolSpec};
use serde_json::json;

use crate::CliError;

/// `fixloop llm` subcommands.
#[derive(Debug, Subcommand)]
pub(crate) enum LlmCommand {
    /// List configured models, routes and allowed endpoints.
    List,
    /// Send a test prompt to a model or task route.
    Test(TestArgs),
}

/// Arguments for `fixloop llm test`.
#[derive(Debug, Args)]
pub(crate) struct TestArgs {
    /// Model name from llm.models (bypasses routes).
    #[arg(long, conflicts_with = "task")]
    model: Option<String>,

    /// Task route to use: triage, dedupe, review, fix, second_review, explain, summarize.
    #[arg(long, default_value = "review", value_parser = parse_task)]
    task: Task,

    /// Prompt to send.
    #[arg(long, default_value = "In one sentence: what is an off-by-one error?")]
    prompt: String,

    /// Also offer a `report_finding` tool and ask the model to call it.
    #[arg(long)]
    tool: bool,

    /// Behave as a `local-only` repository (external models are skipped).
    #[arg(long)]
    local_only: bool,

    /// Maximum output tokens.
    #[arg(long, default_value_t = 256)]
    max_tokens: u32,
}

fn parse_task(s: &str) -> Result<Task, String> {
    serde_json::from_value(json!(s)).map_err(|_| {
        format!("unknown task `{s}` (expected triage, dedupe, embed, review, fix, second_review, explain or summarize)")
    })
}

impl From<LlmError> for CliError {
    fn from(e: LlmError) -> Self {
        Self::new(e.to_string(), e.hint())
    }
}

const TOOL_PROMPT: &str = "Here is a function:\n\n\
    fn last(items: &[i32]) -> i32 { items[items.len()] }\n\n\
    Call the report_finding tool once to report the bug in it.";

fn report_tool() -> ToolSpec {
    ToolSpec {
        name: "report_finding".into(),
        description: "Report one bug found in the code.".into(),
        input_schema: json!({
            "type": "object",
            "required": ["title", "severity"],
            "properties": {
                "title": {"type": "string", "description": "Short title of the bug"},
                "severity": {"type": "string", "enum": ["critical", "high", "medium", "low", "info"]}
            }
        }),
    }
}

pub(crate) async fn run(cmd: &LlmCommand, config: &FixloopConfig) -> Result<(), CliError> {
    let router = Router::from_config(&config.llm, &|k| std::env::var(k).ok())?;
    match cmd {
        LlmCommand::List => {
            list(&router, config);
            Ok(())
        }
        LlmCommand::Test(args) => test(&router, args).await,
    }
}

fn list(router: &Router, config: &FixloopConfig) {
    println!("offline: {}", config.llm.offline);
    println!("\nmodels:");
    let models: Vec<_> = router.models().collect();
    if models.is_empty() {
        println!("  (none; add some under llm.models)");
    }
    for m in models {
        let price = m.price.map_or_else(
            || "no price".to_owned(),
            |p| format!("${}/${} per M tokens", p.input_per_mtok, p.output_per_mtok),
        );
        println!(
            "  {:<14} {:<40} provider={} ({:?}), tools={:?}, {price}",
            m.name,
            m.model_id,
            m.provider.name(),
            m.provider.locality(),
            m.tool_mode,
        );
    }
    println!("\nroutes:");
    for (task, chain) in &config.llm.routes {
        println!("  {:<14} {}", format!("{task:?}"), chain.join(" -> "));
    }
    println!(
        "\nallowed endpoints: {}",
        router.egress_allowlist().join(", ")
    );
}

async fn test(router: &Router, args: &TestArgs) -> Result<(), CliError> {
    let mut req = if args.tool {
        let mut r = CompletionRequest::from_prompt(TOOL_PROMPT, args.max_tokens);
        r.tools = vec![report_tool()];
        r
    } else {
        CompletionRequest::from_prompt(&args.prompt, args.max_tokens)
    };
    req.system = Some("You are a careful code reviewer.".into());
    let egress = if args.local_only {
        DataEgress::LocalOnly
    } else {
        DataEgress::Any
    };
    let ctx = CallContext::new(egress).with_budget(router.job_budget("llm-test"));

    let started = Instant::now();
    let r = match &args.model {
        Some(m) => router.complete_with_model(m, &req, &ctx).await?,
        None => router.complete(args.task, &req, &ctx).await?,
    };
    let elapsed = started.elapsed();

    for f in &r.failed_attempts {
        println!("fallback: {f}");
    }
    println!(
        "model:    {} ({} via {})\ntokens:   {} in / {} out\ncost:     ${:.6}\nlatency:  {:.2}s\nstop:     {:?}",
        r.model,
        r.model_id,
        r.provider,
        r.response.usage.input_tokens,
        r.response.usage.output_tokens,
        r.cost_usd,
        elapsed.as_secs_f64(),
        r.response.stop_reason,
    );
    let text = r.response.text();
    if !text.trim().is_empty() {
        println!("\n{}", text.trim());
    }
    for call in r.response.tool_calls() {
        println!("\ntool call: {}({})", call.name, call.arguments);
    }
    if args.tool && r.response.tool_calls().is_empty() {
        return Err(CliError::new(
            format!("model `{}` did not call the tool", r.model),
            Some("try `tool_mode: json` for this model, or a stronger model".into()),
        ));
    }
    Ok(())
}
