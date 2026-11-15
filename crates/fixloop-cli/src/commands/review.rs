//! `fixloop review`: review local changes, or a GitHub pull request.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use async_trait::async_trait;
use clap::Args;
use fixloop_agents::reviewer::render;
use fixloop_agents::reviewer::{
    PrReview, ReviewInput, ReviewResult, ReviewTrigger, review_diff, review_pull_request,
};
use fixloop_core::domain::Severity;
use fixloop_core::repo_config::REPO_CONFIG_FILE;
use fixloop_core::scm::{
    CheckConclusion, PullRequest, RepoRef, ReviewSubmission, ScmError, ScmProvider,
};
use fixloop_core::{FixloopConfig, Hinted, RepoConfig};
use fixloop_llm::{CallContext, Router};
use fixloop_scm::github::AppAuth;
use fixloop_scm::github::client::GithubApp;

use crate::CliError;

/// Arguments for `fixloop review`.
#[derive(Debug, Args)]
pub(crate) struct ReviewArgs {
    /// Review a GitHub pull request instead of local changes,
    /// e.g. https://github.com/org/repo/pull/42
    #[arg(long)]
    pr: Option<String>,

    /// With --pr: publish the review (comments and check run) on GitHub.
    /// Without it, nothing is posted.
    #[arg(long, requires = "pr")]
    post: bool,

    /// Local mode: branch or commit to compare against (default: the
    /// remote's default branch, else `main`).
    #[arg(long, conflicts_with = "pr")]
    base: Option<String>,

    /// Local mode: repository directory.
    #[arg(long, default_value = ".", conflicts_with = "pr")]
    path: PathBuf,
}

impl From<fixloop_agents::ReviewError> for CliError {
    fn from(e: fixloop_agents::ReviewError) -> Self {
        Self::new(e.to_string(), e.hint())
    }
}

pub(crate) async fn run(args: &ReviewArgs, config: &FixloopConfig) -> Result<(), CliError> {
    let router = Router::from_config(&config.llm, &|k| std::env::var(k).ok())?;
    let result = match &args.pr {
        Some(url) => review_pr(url, args.post, config, &router).await?,
        None => review_local(args, &router).await?,
    };
    let Some(result) = result else { return Ok(()) };
    print_result(&result);
    if result.conclusion() == CheckConclusion::Failure {
        // Distinct code so hooks and scripts can block on serious findings.
        std::process::exit(3);
    }
    Ok(())
}

// ---------- local changes ----------

fn git(dir: &Path, args: &[&str]) -> Result<String, CliError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| {
            CliError::new(
                format!("could not run git: {e}"),
                Some("install git and make sure it is on PATH".into()),
            )
        })?;
    if !out.status.success() {
        return Err(CliError::new(
            format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
            Some("run this inside a git repository, or pass --path".into()),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn default_base(dir: &Path) -> String {
    git(
        dir,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .map(|s| s.trim().to_owned())
    .ok()
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "main".into())
}

async fn review_local(
    args: &ReviewArgs,
    router: &Router,
) -> Result<Option<ReviewResult>, CliError> {
    let dir = &args.path;
    let root = PathBuf::from(git(dir, &["rev-parse", "--show-toplevel"])?.trim());
    let base = args.base.clone().unwrap_or_else(|| default_base(&root));
    let merge_base = git(&root, &["merge-base", &base, "HEAD"]).map_err(|e| {
        CliError::new(
            format!("cannot compare with `{base}`: {}", e.message),
            Some("pass --base with an existing branch or commit".into()),
        )
    })?;
    // Committed and uncommitted changes since the branch point.
    let diff = git(
        &root,
        &[
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--unified=3",
            merge_base.trim(),
        ],
    )?;
    if diff.trim().is_empty() {
        println!("No changes compared with `{base}`.");
        return Ok(None);
    }

    let config = match std::fs::read_to_string(root.join(REPO_CONFIG_FILE)) {
        Ok(text) => RepoConfig::from_yaml(&text, REPO_CONFIG_FILE)?,
        Err(_) => RepoConfig::default(),
    };
    let rules = read_rules(&root.join(&config.rules_dir));
    let mut files = BTreeMap::new();
    for f in fixloop_agents::diff::parse(&diff) {
        if let Some(p) = &f.new_path
            && let Ok(text) = std::fs::read_to_string(root.join(p))
            && text.len() <= 40_000
        {
            files.insert(p.clone(), text);
        }
    }
    let subject = git(&root, &["log", "-1", "--format=%s"]).unwrap_or_default();
    let input = ReviewInput {
        title: subject.trim().to_owned(),
        description: String::new(),
        diff,
        files,
        rules,
        config: config.clone(),
    };
    println!("Reviewing changes against `{base}`…");
    let ctx = CallContext::new(config.data_egress).with_budget(router.job_budget("local-review"));
    Ok(Some(review_diff(router, &input, &ctx).await?))
}

fn read_rules(dir: &Path) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut rules: Vec<(String, String)> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            std::fs::read_to_string(e.path())
                .ok()
                .map(|t| (name, t.chars().take(20_000).collect()))
        })
        .collect();
    rules.sort();
    rules.truncate(20);
    rules
}

// ---------- pull requests ----------

fn parse_pr_url(url: &str) -> Option<(RepoRef, u64)> {
    let rest = url.trim().trim_end_matches('/');
    let rest = rest
        .strip_prefix("https://github.com/")
        .or_else(|| rest.strip_prefix("github.com/"))?;
    let parts: Vec<&str> = rest.split('/').collect();
    match parts.as_slice() {
        [owner, name, "pull", n, ..] => {
            Some((RepoRef::parse(&format!("{owner}/{name}"))?, n.parse().ok()?))
        }
        _ => None,
    }
}

async fn review_pr(
    url: &str,
    post: bool,
    config: &FixloopConfig,
    router: &Router,
) -> Result<Option<ReviewResult>, CliError> {
    let (repo, number) = parse_pr_url(url).ok_or_else(|| {
        CliError::new(
            format!("`{url}` is not a pull request URL"),
            Some("use https://github.com/<owner>/<repo>/pull/<number>".into()),
        )
    })?;
    let g = config.github.as_ref().ok_or_else(|| {
        CliError::new(
            "reviewing a pull request needs the GitHub App",
            Some("run `fixloop github create-app`, then add the `github` section it prints to your config".into()),
        )
    })?;
    let auth = AppAuth::from_file(g.app_id, &g.private_key_path)
        .map_err(|e| CliError::new(e.to_string(), Some("check github.private_key_path".into())))?;
    let app =
        GithubApp::new(auth, g.api_url.clone()).map_err(|e| CliError::new(e.to_string(), None))?;
    let client = app.for_repo(&repo).await.map_err(|e| {
        let e: ScmError = e.into();
        CliError::new(e.to_string(), e.hint())
    })?;

    println!(
        "Reviewing {repo}#{number}{}…",
        if post {
            " (will publish)"
        } else {
            " (dry run: nothing is posted)"
        }
    );
    let outcome = if post {
        review_pull_request(
            &client,
            router,
            &repo,
            number,
            &ReviewTrigger::Automatic,
            None,
        )
        .await?
    } else {
        let dry = DryRun {
            inner: &client,
            posted: Mutex::new(false),
        };
        review_pull_request(&dry, router, &repo, number, &ReviewTrigger::Automatic, None).await?
    };
    match outcome {
        PrReview::Done { result, .. } => {
            if post {
                println!("Published to https://github.com/{repo}/pull/{number}");
            }
            Ok(Some(result))
        }
        PrReview::SkippedDraft => {
            println!("Skipped: the pull request is a draft.");
            Ok(None)
        }
        PrReview::Unauthorized(_) => Ok(None),
    }
}

/// Reads go to GitHub; writes are dropped.
struct DryRun<'a> {
    inner: &'a dyn ScmProvider,
    posted: Mutex<bool>,
}

#[async_trait]
impl ScmProvider for DryRun<'_> {
    async fn pull_request(&self, r: &RepoRef, n: u64) -> Result<PullRequest, ScmError> {
        self.inner.pull_request(r, n).await
    }
    async fn pull_request_diff(&self, r: &RepoRef, n: u64) -> Result<String, ScmError> {
        self.inner.pull_request_diff(r, n).await
    }
    async fn file_at(&self, r: &RepoRef, p: &str, g: &str) -> Result<Option<String>, ScmError> {
        self.inner.file_at(r, p, g).await
    }
    async fn list_dir(&self, r: &RepoRef, p: &str, g: &str) -> Result<Vec<String>, ScmError> {
        self.inner.list_dir(r, p, g).await
    }
    async fn start_check(&self, _: &RepoRef, _: &str, _: &str) -> Result<u64, ScmError> {
        Ok(0)
    }
    async fn finish_check(
        &self,
        _: &RepoRef,
        _: u64,
        _: CheckConclusion,
        _: &str,
        _: &str,
    ) -> Result<(), ScmError> {
        Ok(())
    }
    async fn post_review(&self, _: &RepoRef, _: u64, _: &ReviewSubmission) -> Result<(), ScmError> {
        *self.posted.lock().unwrap_or_else(|e| e.into_inner()) = true;
        Ok(())
    }
    async fn post_comment(&self, _: &RepoRef, _: u64, _: &str) -> Result<(), ScmError> {
        Ok(())
    }
    async fn can_write(&self, r: &RepoRef, u: &str) -> Result<bool, ScmError> {
        self.inner.can_write(r, u).await
    }
}

// ---------- output ----------

fn print_result(r: &ReviewResult) {
    println!();
    if r.findings.is_empty() {
        println!("No problems found.");
    }
    for f in &r.findings {
        let sev = match f.severity {
            Severity::Critical => "CRITICAL",
            Severity::High => "HIGH",
            Severity::Medium => "MEDIUM",
            Severity::Low => "LOW",
            Severity::Info => "INFO",
        };
        println!(
            "{}:{}  {sev} {}  {}",
            f.path,
            f.line,
            f.category.as_str(),
            f.title
        );
        println!("    {}", f.rationale.replace('\n', "\n    "));
        if let Some(fix) = &f.suggested_fix {
            println!("    Fix: {}", fix.replace('\n', "\n    "));
        }
        if let Some(s) = &f.suggestion {
            println!("    Suggested replacement:");
            for l in s.lines() {
                println!("    | {l}");
            }
        }
        println!("    (confidence {:.0}%)\n", f.confidence * 100.0);
    }
    println!("{}", render::check_title(r));
    if !r.model.is_empty() {
        println!(
            "Model {} · {} tokens{}",
            r.model,
            r.usage.total(),
            if r.cost_usd > 0.0 {
                format!(" · ${:.4}", r.cost_usd)
            } else {
                String::new()
            }
        );
    }
    let hidden = r.filtered_out + r.off_diff;
    if hidden > 0 {
        println!("{hidden} lower-confidence or off-diff finding(s) hidden.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pr_url_parsing() {
        let (r, n) = parse_pr_url("https://github.com/bwalia/fixloop-test/pull/12/files").unwrap();
        assert_eq!((r.to_string(), n), ("bwalia/fixloop-test".into(), 12));
        assert!(parse_pr_url("https://github.com/bwalia/fixloop-test/issues/1").is_none());
        assert!(parse_pr_url("https://evil.com/a/b/pull/1").is_none());
    }
}
