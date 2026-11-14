//! Provider implementations.

pub mod anthropic;
pub mod openai;

use url::Url;

use crate::error::LlmError;

/// Append `path` to `base`, keeping any path prefix in `base`
/// (`http://h/v1` + `chat/completions` → `http://h/v1/chat/completions`).
pub(crate) fn endpoint(base: &Url, path: &str) -> Result<Url, LlmError> {
    let mut base = base.clone();
    if !base.path().ends_with('/') {
        let p = format!("{}/", base.path());
        base.set_path(&p);
    }
    base.join(path).map_err(|e| LlmError::Config {
        message: format!("invalid endpoint {base}{path}: {e}"),
        hint: "check base_url".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_keeps_prefix() {
        let b = Url::parse("http://localhost:11434/v1").unwrap();
        assert_eq!(
            endpoint(&b, "chat/completions").unwrap().as_str(),
            "http://localhost:11434/v1/chat/completions"
        );
        let b = Url::parse("https://api.anthropic.com/").unwrap();
        assert_eq!(
            endpoint(&b, "v1/messages").unwrap().as_str(),
            "https://api.anthropic.com/v1/messages"
        );
    }
}
