//! GitHub webhook verification and parsing.

use fixloop_core::scm::RepoRef;
use hmac::{Hmac, KeyInit, Mac};
use serde_json::Value;
use sha2::Sha256;

/// Verify `X-Hub-Signature-256` over the raw request body, in constant time.
pub fn verify_signature(secret: &[u8], body: &[u8], header: &str) -> bool {
    let Some(hex_sig) = header.strip_prefix("sha256=") else {
        return false;
    };
    let Ok(sig) = hex::decode(hex_sig) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&sig).is_ok()
}

/// Compute the signature header value (for tests and local tools).
pub fn sign(secret: &[u8], body: &[u8]) -> String {
    match Hmac::<Sha256>::new_from_slice(secret) {
        Ok(mut mac) => {
            mac.update(body);
            format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
        }
        Err(_) => String::new(),
    }
}

/// Why a review was requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trigger {
    /// PR opened, updated or marked ready.
    PullRequest(String),
    /// `/fixloop review` comment by this user (must be authorised).
    Command(String),
}

/// A review the webhook asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRequest {
    /// Repository.
    pub repo: RepoRef,
    /// PR number.
    pub number: u64,
    /// App installation that sent the event.
    pub installation_id: u64,
    /// Why.
    pub trigger: Trigger,
}

/// Turn a webhook into a review request, if it is one. Parsing errors and
/// irrelevant events return `None`.
pub fn review_request(event: &str, body: &[u8]) -> Option<ReviewRequest> {
    let v: Value = serde_json::from_slice(body).ok()?;
    let repo = RepoRef::parse(v.pointer("/repository/full_name")?.as_str()?)?;
    let installation_id = v.pointer("/installation/id")?.as_u64()?;
    let action = v.get("action")?.as_str()?;
    match event {
        "pull_request" => {
            if !matches!(
                action,
                "opened" | "synchronize" | "reopened" | "ready_for_review"
            ) {
                return None;
            }
            if v.pointer("/pull_request/draft").and_then(Value::as_bool) == Some(true) {
                return None;
            }
            Some(ReviewRequest {
                repo,
                number: v.pointer("/pull_request/number")?.as_u64()?,
                installation_id,
                trigger: Trigger::PullRequest(action.to_owned()),
            })
        }
        "issue_comment" => {
            // Only new comments, only on pull requests, only the exact command.
            if action != "created" || v.pointer("/issue/pull_request").is_none() {
                return None;
            }
            let body = v.pointer("/comment/body")?.as_str()?.trim();
            let first = body.lines().next()?.trim();
            if first != "/fixloop review" {
                return None;
            }
            if v.pointer("/comment/user/type").and_then(Value::as_str) == Some("Bot") {
                return None;
            }
            Some(ReviewRequest {
                repo,
                number: v.pointer("/issue/number")?.as_u64()?,
                installation_id,
                trigger: Trigger::Command(v.pointer("/comment/user/login")?.as_str()?.to_owned()),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn signature_round_trip_and_tamper() {
        let sig = sign(b"s3cret", b"{\"a\":1}");
        assert!(verify_signature(b"s3cret", b"{\"a\":1}", &sig));
        assert!(!verify_signature(b"s3cret", b"{\"a\":2}", &sig));
        assert!(!verify_signature(b"other", b"{\"a\":1}", &sig));
        assert!(!verify_signature(b"s3cret", b"{\"a\":1}", "sha1=abc"));
        assert!(!verify_signature(b"s3cret", b"{\"a\":1}", "sha256=zz"));
    }

    fn pr_event(action: &str, draft: bool) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "action": action,
            "repository": {"full_name": "o/r"},
            "installation": {"id": 7},
            "pull_request": {"number": 3, "draft": draft}
        }))
        .unwrap()
    }

    #[test]
    fn pull_request_events() {
        let r = review_request("pull_request", &pr_event("opened", false)).unwrap();
        assert_eq!((r.number, r.installation_id), (3, 7));
        assert!(review_request("pull_request", &pr_event("closed", false)).is_none());
        assert!(review_request("pull_request", &pr_event("opened", true)).is_none());
    }

    fn comment(body: &str, on_pr: bool, user_type: &str) -> Vec<u8> {
        let mut issue = json!({"number": 5});
        if on_pr {
            issue["pull_request"] = json!({"url": "x"});
        }
        serde_json::to_vec(&json!({
            "action": "created",
            "repository": {"full_name": "o/r"},
            "installation": {"id": 7},
            "issue": issue,
            "comment": {"body": body, "user": {"login": "alice", "type": user_type}}
        }))
        .unwrap()
    }

    #[test]
    fn review_command() {
        let r = review_request(
            "issue_comment",
            &comment("/fixloop review\nplease", true, "User"),
        )
        .unwrap();
        assert_eq!(r.trigger, Trigger::Command("alice".into()));
        assert!(
            review_request(
                "issue_comment",
                &comment("please /fixloop review", true, "User")
            )
            .is_none()
        );
        assert!(
            review_request("issue_comment", &comment("/fixloop review", false, "User")).is_none()
        );
        assert!(
            review_request("issue_comment", &comment("/fixloop review", true, "Bot")).is_none()
        );
        assert!(
            review_request("issue_comment", &comment("/fixloop reviewx", true, "User")).is_none()
        );
    }
}
