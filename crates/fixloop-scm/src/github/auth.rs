//! GitHub App authentication: app JWTs and cached installation tokens.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Serialize;

use crate::github::client::GithubError;

/// Credentials of one GitHub App.
pub struct AppAuth {
    app_id: u64,
    key: EncodingKey,
    tokens: Mutex<HashMap<u64, (String, Instant)>>,
}

impl std::fmt::Debug for AppAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppAuth")
            .field("app_id", &self.app_id)
            .finish_non_exhaustive()
    }
}

#[derive(Serialize)]
struct Claims {
    iat: u64,
    exp: u64,
    iss: String,
}

/// Installation tokens live 60 minutes; refresh well before that.
const TOKEN_TTL: Duration = Duration::from_secs(50 * 60);

impl AppAuth {
    /// Create from the App id and its PEM private key.
    pub fn new(app_id: u64, pem: &[u8]) -> Result<Self, GithubError> {
        let key = EncodingKey::from_rsa_pem(pem)
            .map_err(|e| GithubError::Auth(format!("invalid GitHub App private key: {e}")))?;
        Ok(Self {
            app_id,
            key,
            tokens: Mutex::new(HashMap::new()),
        })
    }

    /// Load the private key from a file.
    pub fn from_file(app_id: u64, path: &std::path::Path) -> Result<Self, GithubError> {
        let pem = std::fs::read(path).map_err(|e| {
            GithubError::Auth(format!(
                "could not read private key `{}`: {e}",
                path.display()
            ))
        })?;
        Self::new(app_id, &pem)
    }

    /// App id.
    pub fn app_id(&self) -> u64 {
        self.app_id
    }

    /// A short-lived JWT identifying the App itself.
    pub fn jwt(&self) -> Result<String, GithubError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| GithubError::Auth(e.to_string()))?
            .as_secs();
        // Backdate for clock drift; GitHub allows at most 10 minutes.
        let claims = Claims {
            iat: now.saturating_sub(60),
            exp: now + 9 * 60,
            iss: self.app_id.to_string(),
        };
        jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, &self.key)
            .map_err(|e| GithubError::Auth(format!("could not sign app JWT: {e}")))
    }

    pub(crate) fn cached_token(&self, installation: u64) -> Option<String> {
        let map = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&installation)
            .filter(|(_, at)| at.elapsed() < TOKEN_TTL)
            .map(|(t, _)| t.clone())
    }

    pub(crate) fn store_token(&self, installation: u64, token: String) {
        let mut map = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(installation, (token, Instant::now()));
    }
}
