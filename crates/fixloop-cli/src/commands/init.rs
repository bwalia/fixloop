//! `fixloop init`: scaffold repository configuration.

use std::fs;
use std::path::{Path, PathBuf};

use clap::Args;
use fixloop_core::RepoConfig;
use fixloop_core::repo_config::REPO_CONFIG_FILE;

use crate::CliError;

/// Arguments for `fixloop init`.
#[derive(Debug, Args)]
pub(crate) struct InitArgs {
    /// Repository root to initialise.
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Overwrite an existing .fixloop.yaml.
    #[arg(long)]
    force: bool,
}

const RULE_EXAMPLE: &str = r#"# Example rule: no unwrap on user input

Each Markdown file in this folder is one team rule. Fixloop reads these rules
when it reviews code and applies them like a teammate would.

Write the rule in plain language. Say what to flag, why, and what to do instead.

- Flag `.unwrap()` or `.expect()` on values that come from user input,
  network calls or files.
- Why: these panic at runtime and crash the service.
- Instead: return an error with context.

Delete this file or replace it with your own rules.
"#;

pub(crate) fn run(args: &InitArgs) -> Result<(), CliError> {
    if !args.path.is_dir() {
        return Err(CliError::new(
            format!("`{}` is not a directory", args.path.display()),
            Some("run `fixloop init` from your repository root, or pass its path".into()),
        ));
    }

    let config_path = args.path.join(REPO_CONFIG_FILE);
    if config_path.exists() && !args.force {
        return Err(CliError::new(
            format!("`{}` already exists", config_path.display()),
            Some("pass --force to overwrite it".into()),
        ));
    }
    write(&config_path, RepoConfig::template())?;

    let rules_dir = args.path.join(".fixloop").join("rules");
    fs::create_dir_all(&rules_dir).map_err(|e| io_error(&rules_dir, &e))?;
    let example = rules_dir.join("example-no-unwrap.md");
    if !example.exists() {
        write(&example, RULE_EXAMPLE)?;
    }

    println!("Created {}", config_path.display());
    println!("Created {}", example.display());
    println!("Next: review the settings, commit both files, then install the Fixloop GitHub App.");
    Ok(())
}

fn write(path: &Path, contents: &str) -> Result<(), CliError> {
    fs::write(path, contents).map_err(|e| io_error(path, &e))
}

fn io_error(path: &Path, e: &std::io::Error) -> CliError {
    CliError::new(
        format!("could not write `{}`: {e}", path.display()),
        Some("check that the folder exists and you have write permission".into()),
    )
}
