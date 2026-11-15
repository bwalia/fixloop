//! Secret detection and redaction.
//!
//! Applied to everything before it is sent to a model, and to everything
//! Fixloop posts. Patterns favour precision: a missed exotic token is
//! possible, but common credential formats are always caught.

use std::sync::OnceLock;

use regex::Regex;

/// A detected secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretMatch {
    /// Kind of secret, e.g. `github-token`.
    pub kind: &'static str,
    /// Byte range in the scanned text.
    pub range: std::ops::Range<usize>,
}

const PATTERNS: &[(&str, &str)] = &[
    (
        "private-key",
        r"-----BEGIN (?:RSA |EC |DSA |OPENSSH |PGP |ENCRYPTED )?PRIVATE KEY(?: BLOCK)?-----[\s\S]*?(?:-----END [A-Z ]*PRIVATE KEY(?: BLOCK)?-----|\z)",
    ),
    (
        "github-token",
        r"\b(?:gh[pousr]_[A-Za-z0-9]{36,255}|github_pat_[A-Za-z0-9_]{60,255})\b",
    ),
    (
        "aws-access-key-id",
        r"\b(?:AKIA|ASIA|ABIA|ACCA)[A-Z0-9]{16}\b",
    ),
    ("anthropic-key", r"\bsk-ant-[A-Za-z0-9_\-]{20,}"),
    (
        "openai-key",
        r"\bsk-(?:proj-|svcacct-|admin-)?[A-Za-z0-9_\-]{32,}",
    ),
    ("slack-token", r"\bxox[abposr]-[A-Za-z0-9-]{10,}"),
    (
        "slack-webhook",
        r"https://hooks\.slack\.com/services/[A-Za-z0-9/_\-]+",
    ),
    ("google-api-key", r"\bAIza[0-9A-Za-z_\-]{35}\b"),
    ("stripe-key", r"\b(?:sk|rk)_live_[0-9a-zA-Z]{24,}\b"),
    ("gitlab-token", r"\bglpat-[A-Za-z0-9_\-]{20,}\b"),
    (
        "jwt",
        r"\beyJ[A-Za-z0-9_\-]{10,}\.eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\b",
    ),
    (
        "url-credentials",
        r"(?i)\b[a-z][a-z0-9+.\-]*://[^\s/:@]+:[^\s/@]{3,}@[^\s/]+",
    ),
    (
        "assigned-secret",
        r#"(?i)\b(?:password|passwd|secret|api[_-]?key|access[_-]?token|auth[_-]?token|client[_-]?secret)\b["']?\s*[:=]\s*["'][^"'\s]{8,}["']"#,
    ),
];

fn compiled() -> &'static [(&'static str, Regex)] {
    static RE: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    RE.get_or_init(|| {
        PATTERNS
            .iter()
            // Patterns are constants covered by tests; a bad one is skipped
            // rather than crashing the process.
            .filter_map(|(k, p)| Regex::new(p).ok().map(|r| (*k, r)))
            .collect()
    })
}

/// Find secrets in `text`. Overlapping matches are merged.
pub fn scan(text: &str) -> Vec<SecretMatch> {
    let mut found: Vec<SecretMatch> = compiled()
        .iter()
        .flat_map(|(kind, re)| {
            re.find_iter(text).map(move |m| SecretMatch {
                kind,
                range: m.range(),
            })
        })
        .collect();
    found.sort_by_key(|m| (m.range.start, std::cmp::Reverse(m.range.end)));
    let mut merged: Vec<SecretMatch> = Vec::new();
    for m in found {
        match merged.last_mut() {
            Some(last) if m.range.start < last.range.end => {
                last.range.end = last.range.end.max(m.range.end);
            }
            _ => merged.push(m),
        }
    }
    merged
}

/// Replace every secret with `[REDACTED:<kind>]`.
pub fn redact(text: &str) -> String {
    let matches = scan(text);
    if matches.is_empty() {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    for m in matches {
        out.push_str(&text[pos..m.range.start]);
        out.push_str("[REDACTED:");
        out.push_str(m.kind);
        out.push(']');
        pos = m.range.end;
    }
    out.push_str(&text[pos..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_patterns_compile() {
        assert_eq!(compiled().len(), PATTERNS.len());
    }

    #[test]
    fn detects_common_secrets() {
        let samples = [
            (
                "github-token",
                "token = ghp_0123456789abcdefghijABCDEFGHIJ0123456789",
            ),
            (
                "github-token",
                "github_pat_11AAAAAAA0123456789_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123",
            ),
            ("aws-access-key-id", "AKIAIOSFODNN7EXAMPLE"),
            (
                "anthropic-key",
                "sk-ant-api03-abcdefghijklmnopqrstuvwxyz012345",
            ),
            ("slack-token", "xoxb-1234567890-abcdefghij"),
            (
                "private-key",
                "-----BEGIN RSA PRIVATE KEY-----\nMIIabc\n-----END RSA PRIVATE KEY-----",
            ),
            (
                "url-credentials",
                "https://user:hunter2pass@example.com/repo.git",
            ),
            ("assigned-secret", r#"password = "correct-horse-battery""#),
        ];
        for (kind, s) in samples {
            let m = scan(s);
            assert!(
                m.iter().any(|m| m.kind == kind),
                "{kind} not found in {s}: {m:?}"
            );
        }
    }

    #[test]
    fn redacts_and_keeps_surrounding_text() {
        let out = redact("key=AKIAIOSFODNN7EXAMPLE; ok");
        assert_eq!(out, "key=[REDACTED:aws-access-key-id]; ok");
    }

    #[test]
    fn ordinary_code_is_untouched() {
        let code = "fn main() { let password = read_password(); let x = \"hello\"; }";
        assert_eq!(redact(code), code);
    }

    #[test]
    fn unterminated_private_key_is_still_redacted() {
        let out = redact("-----BEGIN PRIVATE KEY-----\nMIIabc");
        assert!(!out.contains("MIIabc"), "{out}");
    }
}
