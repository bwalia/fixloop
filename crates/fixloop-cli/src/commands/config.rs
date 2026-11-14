//! `fixloop config`: inspect and validate configuration.

use std::path::{Path, PathBuf};

use clap::Subcommand;
use fixloop_core::repo_config::REPO_CONFIG_FILE;
use fixloop_core::{FixloopConfig, RepoConfig};

use crate::CliError;

/// `fixloop config` subcommands.
#[derive(Debug, Subcommand)]
pub(crate) enum ConfigCommand {
    /// Validate the server config (file + FIXLOOP_* environment variables).
    Validate,
    /// Print the effective server config as YAML.
    Show,
    /// Validate a repository's .fixloop.yaml.
    ValidateRepo {
        /// Path to the file, or to the repository root.
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

pub(crate) fn run(cmd: &ConfigCommand, config: Option<&Path>) -> Result<(), CliError> {
    match cmd {
        ConfigCommand::Validate => {
            FixloopConfig::load(config)?;
            println!("Server config is valid.");
            Ok(())
        }
        ConfigCommand::Show => {
            let cfg = FixloopConfig::load(config)?;
            let yaml = serde_saphyr::to_string(&cfg)
                .map_err(|e| CliError::new(format!("could not render config: {e}"), None))?;
            print!("{yaml}");
            Ok(())
        }
        ConfigCommand::ValidateRepo { path } => {
            let file = if path.is_dir() {
                path.join(REPO_CONFIG_FILE)
            } else {
                path.clone()
            };
            let text = std::fs::read_to_string(&file).map_err(|e| {
                CliError::new(
                    format!("could not read `{}`: {e}", file.display()),
                    Some("run `fixloop init` to create one".into()),
                )
            })?;
            RepoConfig::from_yaml(&text, &file.display().to_string())?;
            println!("{} is valid.", file.display());
            Ok(())
        }
    }
}
