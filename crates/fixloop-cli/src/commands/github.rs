//! `fixloop github`: register the GitHub App with the manifest flow.

use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::Router;
use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::get;
use clap::{Args, Subcommand};
use fixloop_scm::github::manifest;
use serde_json::json;
use tokio::sync::{mpsc, oneshot};
use url::Url;

use crate::CliError;

/// `fixloop github` subcommands.
#[derive(Debug, Subcommand)]
pub(crate) enum GithubCommand {
    /// Register a new GitHub App for Fixloop (opens your browser).
    CreateApp(CreateAppArgs),
}

/// Arguments for `fixloop github create-app`.
#[derive(Debug, Args)]
pub(crate) struct CreateAppArgs {
    /// App name. Must be unique on GitHub.
    #[arg(long)]
    name: String,
    /// Create the App in this organisation instead of your account.
    #[arg(long)]
    org: Option<String>,
    /// Public webhook URL, if Fixloop is already reachable. Without it,
    /// webhooks start disabled.
    #[arg(long)]
    webhook_url: Option<Url>,
    /// Where to save the App credentials.
    #[arg(long)]
    out_dir: Option<PathBuf>,
    /// Print the URL instead of opening a browser.
    #[arg(long)]
    no_browser: bool,
}

pub(crate) fn run(cmd: &GithubCommand) -> Result<(), CliError> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::new(format!("could not start async runtime: {e}"), None))?;
    match cmd {
        GithubCommand::CreateApp(args) => rt.block_on(create_app(args)),
    }
}

fn random_hex(bytes: usize) -> Result<String, CliError> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf)
        .map_err(|e| CliError::new(format!("no secure randomness available: {e}"), None))?;
    Ok(hex::encode(buf))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[derive(Clone)]
struct FlowState {
    page: String,
    state: String,
    done: mpsc::Sender<String>,
}

#[derive(serde::Deserialize)]
struct Callback {
    code: Option<String>,
    state: Option<String>,
}

async fn create_app(args: &CreateAppArgs) -> Result<(), CliError> {
    let out_dir = match &args.out_dir {
        Some(d) => d.clone(),
        None => default_config_dir()?,
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| CliError::new(format!("could not open a local port: {e}"), None))?;
    let addr = listener
        .local_addr()
        .map_err(|e| CliError::new(e.to_string(), None))?;
    let redirect = Url::parse(&format!("http://{addr}/callback"))
        .map_err(|e| CliError::new(e.to_string(), None))?;

    let state = random_hex(16)?;
    let manifest = manifest::manifest(&args.name, &redirect, args.webhook_url.as_ref());
    let action = manifest::creation_url(args.org.as_deref(), &state);
    let page = format!(
        r#"<!doctype html><html><body style="font-family:sans-serif;max-width:40em;margin:4em auto">
<h1>Create the Fixloop GitHub App</h1>
<p>GitHub will show the App's name and permissions. Click <b>Create GitHub App</b> there.</p>
<form id="f" action="{action}" method="post">
<input type="hidden" name="manifest" value="{manifest}">
<button type="submit">Continue to GitHub</button></form>
<script>document.getElementById('f').submit()</script></body></html>"#,
        action = html_escape(&action),
        manifest = html_escape(&manifest.to_string()),
    );

    let (tx, mut rx) = mpsc::channel::<String>(1);
    let flow = FlowState {
        page,
        state,
        done: tx,
    };
    let app = Router::new()
        .route(
            "/",
            get(|State(s): State<FlowState>| async move { Html(s.page) }),
        )
        .route("/callback", get(callback))
        .with_state(flow);
    let (stop_tx, stop_rx) = oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stop_rx.await;
            })
            .await
    });

    let start = format!("http://{addr}/");
    println!("Opening {start} to create the GitHub App `{}`.", args.name);
    println!("If no browser opens, visit that URL yourself.");
    if !args.no_browser {
        let _ = open::that(&start);
    }

    let code = tokio::time::timeout(Duration::from_secs(15 * 60), rx.recv())
        .await
        .map_err(|_| {
            CliError::new(
                "timed out waiting for GitHub (15 minutes)",
                Some("run the command again".into()),
            )
        })?
        .ok_or_else(|| CliError::new("the local callback server stopped", None))?;
    let _ = stop_tx.send(());
    let _ = server.await;

    let api =
        Url::parse("https://api.github.com/").map_err(|e| CliError::new(e.to_string(), None))?;
    let created = manifest::convert_code(&api, &code).await.map_err(|e| {
        CliError::new(
            format!("GitHub did not return the App credentials: {e}"),
            Some("the code is single-use and expires after an hour; run the command again".into()),
        )
    })?;

    std::fs::create_dir_all(&out_dir).map_err(|e| io_err(&out_dir, &e))?;
    let key_path = out_dir.join(format!("{}.private-key.pem", created.slug));
    write_private(&key_path, created.pem.as_bytes())?;
    let webhook_secret = match &created.webhook_secret {
        Some(s) if !s.is_empty() => s.clone(),
        _ => random_hex(32)?,
    };
    let info_path = out_dir.join(format!("{}.json", created.slug));
    let info = json!({
        "app_id": created.id,
        "slug": created.slug,
        "html_url": created.html_url,
        "private_key_path": key_path,
        "webhook_secret": webhook_secret,
    });
    write_private(
        &info_path,
        serde_json::to_string_pretty(&info)
            .unwrap_or_default()
            .as_bytes(),
    )?;

    let install = format!("https://github.com/apps/{}/installations/new", created.slug);
    println!(
        "\nCreated GitHub App `{}` (id {}).",
        created.slug, created.id
    );
    println!(
        "Private key and webhook secret saved (owner-only) in {}",
        out_dir.display()
    );
    println!("\nAdd this to fixloop.yaml:\n");
    println!(
        "github:\n  app_id: {}\n  private_key_path: {}",
        created.id,
        key_path.display()
    );
    println!(
        "\nand export the webhook secret from {}:",
        info_path.display()
    );
    println!(
        "  export FIXLOOP_GITHUB_WEBHOOK_SECRET=$(jq -r .webhook_secret {})",
        info_path.display()
    );
    println!("\nNext: install the App on your repositories: {install}");
    if !args.no_browser {
        let _ = open::that(&install);
    }
    Ok(())
}

async fn callback(State(s): State<FlowState>, Query(q): Query<Callback>) -> Html<&'static str> {
    match (q.code, q.state) {
        (Some(code), Some(state)) if state == s.state => {
            let _ = s.done.send(code).await;
            Html(
                "<p style='font-family:sans-serif'>GitHub App created. You can close this tab and return to the terminal.</p>",
            )
        }
        _ => Html(
            "<p style='font-family:sans-serif'>Invalid or expired request. Run <code>fixloop github create-app</code> again.</p>",
        ),
    }
}

fn default_config_dir() -> Result<PathBuf, CliError> {
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(x).join("fixloop"));
    }
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".config").join("fixloop"))
        .ok_or_else(|| {
            CliError::new(
                "cannot find your home directory",
                Some("pass --out-dir".into()),
            )
        })
}

fn write_private(path: &Path, data: &[u8]) -> Result<(), CliError> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path).map_err(|e| io_err(path, &e))?;
    f.write_all(data).map_err(|e| io_err(path, &e))
}

fn io_err(path: &Path, e: &std::io::Error) -> CliError {
    CliError::new(
        format!("could not write `{}`: {e}", path.display()),
        Some("check permissions, or pass --out-dir".into()),
    )
}
