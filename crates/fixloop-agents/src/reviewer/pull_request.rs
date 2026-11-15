//! Review a pull request on a code host: check run, config and rules from
//! the base branch, review, and publish.

use std::collections::BTreeMap;

use fixloop_core::RepoConfig;
use fixloop_core::repo_config::REPO_CONFIG_FILE;
use fixloop_core::scm::{CheckConclusion, RepoRef, ReviewComment, ReviewSubmission, ScmProvider};
use fixloop_llm::{BudgetAccount, CallContext, Router};

use super::render;
use super::review::{ReviewInput, ReviewResult, review_diff};
use crate::error::ReviewError;

/// Why a review was requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewTrigger {
    /// Automatic (PR opened or updated).
    Automatic,
    /// `/fixloop review` by this user; must have write access.
    Command(String),
}

/// What happened.
#[derive(Debug)]
pub enum PrReview {
    /// Reviewed and published.
    Done {
        /// Head commit reviewed.
        head_sha: String,
        /// The review.
        result: ReviewResult,
    },
    /// The requester may not trigger reviews.
    Unauthorized(String),
    /// Draft PRs are not reviewed automatically.
    SkippedDraft,
}

const MAX_RULE_FILES: usize = 20;
const MAX_RULE_CHARS: usize = 20_000;
const MAX_CONTEXT_FILE_CHARS: usize = 40_000;

/// Review pull request `number` and publish the result.
pub async fn review_pull_request(
    scm: &dyn ScmProvider,
    router: &Router,
    repo: &RepoRef,
    number: u64,
    trigger: &ReviewTrigger,
    job_budget: Option<std::sync::Arc<BudgetAccount>>,
) -> Result<PrReview, ReviewError> {
    if let ReviewTrigger::Command(user) = trigger
        && !scm.can_write(repo, user).await?
    {
        scm.post_comment(
            repo,
            number,
            "Only people with write access to this repository can run `/fixloop` commands.",
        )
        .await?;
        return Ok(PrReview::Unauthorized(user.clone()));
    }

    let pr = scm.pull_request(repo, number).await?;
    if pr.draft && *trigger == ReviewTrigger::Automatic {
        return Ok(PrReview::SkippedDraft);
    }
    let check_id = scm
        .start_check(repo, &pr.head_sha, render::CHECK_NAME)
        .await?;

    let outcome = run(scm, router, repo, &pr, job_budget).await;
    match outcome {
        Ok(result) => {
            if !result.findings.is_empty() {
                let review = ReviewSubmission {
                    commit_sha: pr.head_sha.clone(),
                    body: render::summary(&result),
                    comments: result
                        .findings
                        .iter()
                        .map(|f| ReviewComment {
                            path: f.path.clone(),
                            line: f.end_line.unwrap_or(f.line),
                            start_line: f.end_line.map(|_| f.line),
                            body: render::comment_body(f),
                        })
                        .collect(),
                };
                scm.post_review(repo, number, &review).await?;
            }
            scm.finish_check(
                repo,
                check_id,
                result.conclusion(),
                &render::check_title(&result),
                &render::summary(&result),
            )
            .await?;
            Ok(PrReview::Done {
                head_sha: pr.head_sha,
                result,
            })
        }
        Err(e) => {
            // Never leave the check spinning; report what went wrong.
            let hint = fixloop_core::Hinted::hint(&e).unwrap_or_default();
            let _ = scm
                .finish_check(
                    repo,
                    check_id,
                    CheckConclusion::Neutral,
                    "Fixloop could not complete the review",
                    &format!("**Error:** {e}\n\n**What to do:** {hint}"),
                )
                .await;
            Err(e)
        }
    }
}

async fn run(
    scm: &dyn ScmProvider,
    router: &Router,
    repo: &RepoRef,
    pr: &fixloop_core::scm::PullRequest,
    job_budget: Option<std::sync::Arc<BudgetAccount>>,
) -> Result<ReviewResult, ReviewError> {
    // Settings and rules come from the base commit, so a PR cannot weaken
    // its own review.
    let config = match scm.file_at(repo, REPO_CONFIG_FILE, &pr.base_sha).await? {
        Some(text) => RepoConfig::from_yaml(&text, REPO_CONFIG_FILE)?,
        None => RepoConfig::default(),
    };
    let mut rules = Vec::new();
    let mut names = scm.list_dir(repo, &config.rules_dir, &pr.base_sha).await?;
    names.retain(|n| n.ends_with(".md"));
    names.sort();
    for name in names.into_iter().take(MAX_RULE_FILES) {
        let path = format!("{}/{name}", config.rules_dir.trim_end_matches('/'));
        if let Some(text) = scm.file_at(repo, &path, &pr.base_sha).await? {
            rules.push((name, text.chars().take(MAX_RULE_CHARS).collect()));
        }
    }

    let diff = scm.pull_request_diff(repo, pr.number).await?;
    let mut files = BTreeMap::new();
    for f in crate::diff::parse(&diff) {
        if f.binary || f.new_path.is_none() {
            continue;
        }
        let path = f.path().to_owned();
        if let Some(text) = scm.file_at(repo, &path, &pr.head_sha).await?
            && text.len() <= MAX_CONTEXT_FILE_CHARS
        {
            files.insert(path, text);
        }
    }

    let input = ReviewInput {
        title: pr.title.clone(),
        description: pr.body.clone(),
        diff,
        files,
        rules,
        config: config.clone(),
    };
    let mut ctx = CallContext::new(config.data_egress);
    if let Some(b) = job_budget {
        ctx = ctx.with_budget(b);
    }
    review_diff(router, &input, &ctx).await
}
