//! The `fixloop` command-line tool.

mod commands;
mod telemetry;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use fixloop_core::config::LogFormat;
use fixloop_core::{ConfigError, FixloopConfig, Hinted};

/// Fixloop closes the bug loop: review, find, file, fix, check, close.
#[derive(Debug, Parser)]
#[command(name = "fixloop", version)]
struct Cli {
    #[command(flatten)]
    global: GlobalArgs,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Args)]
struct GlobalArgs {
    /// Path to the server config file (fixloop.yaml).
    #[arg(long, short = 'c', global = true, env = "FIXLOOP_CONFIG")]
    config: Option<PathBuf>,

    /// Override the log format.
    #[arg(long, global = true, value_parser = parse_log_format)]
    log_format: Option<LogFormat>,

    /// Override the log filter, e.g. `debug` or `fixloop=trace,info`.
    #[arg(long, global = true)]
    log_level: Option<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Add a .fixloop.yaml and a rules folder to a repository.
    Init(commands::init::InitArgs),
    /// Inspect and validate configuration.
    #[command(subcommand)]
    Config(commands::config::ConfigCommand),
    /// Inspect model routing and send test prompts.
    #[command(subcommand)]
    Llm(commands::llm::LlmCommand),
    /// Run the Fixloop server, worker and dashboard.
    Serve,
    /// Review local changes before you push.
    Review,
    /// Run the Bug Fixer on an issue URL.
    Fix {
        /// Issue URL, e.g. https://github.com/org/repo/issues/42
        issue_url: String,
    },
}

fn parse_log_format(s: &str) -> Result<LogFormat, String> {
    match s {
        "pretty" => Ok(LogFormat::Pretty),
        "json" => Ok(LogFormat::Json),
        other => Err(format!(
            "unknown log format `{other}` (expected `pretty` or `json`)"
        )),
    }
}

/// An error shown to the user, with an optional next step.
#[derive(Debug)]
pub(crate) struct CliError {
    message: String,
    hint: Option<String>,
    code: u8,
}

impl CliError {
    pub(crate) fn new(message: impl Into<String>, hint: Option<String>) -> Self {
        Self {
            message: message.into(),
            hint,
            code: 1,
        }
    }

    fn not_yet(what: &str, phase: u8) -> Self {
        Self {
            message: format!("`fixloop {what}` is not implemented yet"),
            hint: Some(format!(
                "it is planned for Phase {phase}; see docs/PLAN.md and ROADMAP.md"
            )),
            code: 2,
        }
    }
}

impl From<ConfigError> for CliError {
    fn from(e: ConfigError) -> Self {
        Self::new(e.to_string(), e.hint())
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e.message);
            if let Some(hint) = e.hint {
                eprintln!("hint: {hint}");
            }
            ExitCode::from(e.code)
        }
    }
}

fn run(cli: Cli) -> Result<(), CliError> {
    // Commands that work without a valid server config.
    match &cli.command {
        Command::Init(args) => return commands::init::run(args),
        Command::Config(cmd) => return commands::config::run(cmd, cli.global.config.as_deref()),
        _ => {}
    }

    let mut config = FixloopConfig::load(cli.global.config.as_deref())?;
    if let Some(f) = cli.global.log_format {
        config.telemetry.log_format = f;
    }
    if let Some(l) = cli.global.log_level {
        config.telemetry.log_level = l;
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::new(format!("could not start async runtime: {e}"), None))?;

    runtime.block_on(async {
        let _guard = telemetry::init(&config.telemetry)?;
        match &cli.command {
            Command::Llm(cmd) => commands::llm::run(cmd, &config).await,
            Command::Serve => Err(CliError::not_yet("serve", 2)),
            Command::Review => Err(CliError::not_yet("review", 2)),
            Command::Fix { .. } => Err(CliError::not_yet("fix", 4)),
            Command::Init(_) | Command::Config(_) => Ok(()),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
