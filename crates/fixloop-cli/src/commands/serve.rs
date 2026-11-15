//! `fixloop serve`: API, webhooks and workers.

use std::sync::Arc;

use fixloop_core::FixloopConfig;
use fixloop_core::config::Role;
use fixloop_llm::Router;
use fixloop_scm::github::AppAuth;
use fixloop_scm::github::client::GithubApp;
use fixloop_server::AppState;
use fixloop_store::Store;
use tokio::sync::watch;

use crate::CliError;

pub(crate) async fn run(config: &FixloopConfig) -> Result<(), CliError> {
    let store = Store::connect(&config.database.url)
        .await
        .map_err(|e| CliError::new(e.to_string(), fixloop_core::Hinted::hint(&e)))?;
    let router = Arc::new(Router::from_config(&config.llm, &|k| {
        std::env::var(k).ok()
    })?);
    let (github, webhook_secret) = match &config.github {
        Some(g) => {
            let auth = AppAuth::from_file(g.app_id, &g.private_key_path).map_err(|e| {
                CliError::new(e.to_string(), Some("check github.private_key_path".into()))
            })?;
            let app = GithubApp::new(auth, g.api_url.clone())
                .map_err(|e| CliError::new(e.to_string(), None))?;
            let secret = std::env::var(&g.webhook_secret_env)
                .ok()
                .filter(|s| !s.is_empty());
            if secret.is_none() {
                tracing::warn!(var = %g.webhook_secret_env, "webhook secret not set; GitHub webhooks will be refused");
            }
            (Some(app), secret.map(|s| Arc::new(s.into_bytes())))
        }
        None => {
            tracing::warn!("no `github` section in config; GitHub integration is off");
            (None, None)
        }
    };
    let state = AppState {
        store,
        github,
        webhook_secret,
        router,
    };

    let (stop_tx, stop_rx) = watch::channel(false);
    let mut workers = Vec::new();
    if matches!(config.server.role, Role::All | Role::Worker) {
        for i in 0..config.server.workers.max(1) {
            let id = format!("{}-{}-{i}", hostname(), std::process::id());
            workers.push(tokio::spawn(fixloop_server::worker::run(
                state.clone(),
                id,
                stop_rx.clone(),
            )));
        }
    }

    let shutdown = async {
        let ctrl_c = tokio::signal::ctrl_c();
        #[cfg(unix)]
        {
            let mut term =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(t) => t,
                    Err(_) => {
                        let _ = ctrl_c.await;
                        return;
                    }
                };
            tokio::select! { _ = ctrl_c => {}, _ = term.recv() => {} }
        }
        #[cfg(not(unix))]
        let _ = ctrl_c.await;
    };

    if matches!(config.server.role, Role::All | Role::Api) {
        let listener = tokio::net::TcpListener::bind(&config.server.bind)
            .await
            .map_err(|e| {
                CliError::new(
                    format!("could not listen on {}: {e}", config.server.bind),
                    Some("change server.bind or stop whatever uses that port".into()),
                )
            })?;
        tracing::info!(addr = %config.server.bind, workers = workers.len(), "fixloop server listening");
        axum::serve(listener, fixloop_server::http::routes(state))
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|e| CliError::new(format!("server error: {e}"), None))?;
    } else {
        tracing::info!(workers = workers.len(), "fixloop worker running");
        shutdown.await;
    }
    tracing::info!("shutting down; finishing current jobs");
    let _ = stop_tx.send(true);
    for w in workers {
        let _ = w.await;
    }
    Ok(())
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "fixloop".into())
}
