//! The review step: diff in, validated findings out. No code-host I/O here,
//! so it serves both PR reviews and `fixloop review` on local changes.

use std::collections::{BTreeMap, BTreeSet};

use fixloop_core::RepoConfig;
use fixloop_core::config::Task;
use fixloop_core::domain::{Category, Finding, Severity};
use fixloop_core::scm::CheckConclusion;
use fixloop_core::secrets;
use fixloop_llm::json_tools::extract_json_object;
use fixloop_llm::{CallContext, CompletionRequest, Message, Router, Usage};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::prompt;
use crate::diff::{self, FileDiff};
use crate::error::ReviewError;

/// Files never worth reviewing.
const DEFAULT_IGNORES: &[&str] = &[
    "**/Cargo.lock",
    "**/package-lock.json",
    "**/yarn.lock",
    "**/pnpm-lock.yaml",
    "**/go.sum",
    "**/poetry.lock",
    "**/uv.lock",
    "**/Gemfile.lock",
    "**/composer.lock",
    "**/*.min.js",
    "**/*.min.css",
    "**/*.svg",
];

/// Character budget for the diff part of the prompt.
const MAX_DIFF_CHARS: usize = 150_000;
/// Character budget for full-file context.
const MAX_CONTEXT_CHARS: usize = 80_000;
/// Most findings posted on one review.
const MAX_FINDINGS: usize = 25;

/// What to review.
#[derive(Debug, Clone, Default)]
pub struct ReviewInput {
    /// PR title (untrusted).
    pub title: String,
    /// PR description (untrusted).
    pub description: String,
    /// Unified diff.
    pub diff: String,
    /// Full new contents of changed files, for context (untrusted).
    pub files: BTreeMap<String, String>,
    /// Team rules: (file name, text), from the default branch.
    pub rules: Vec<(String, String)>,
    /// Repository settings.
    pub config: RepoConfig,
}

/// What the review found and how.
#[derive(Debug, Clone)]
pub struct ReviewResult {
    /// Findings to publish, most severe first.
    pub findings: Vec<Finding>,
    /// Findings removed by the confidence or severity threshold.
    pub filtered_out: usize,
    /// Findings removed because they did not point at a line in the diff.
    pub off_diff: usize,
    /// Model items that could not be understood.
    pub malformed: usize,
    /// Files not sent to the model (ignored, binary, deleted or over budget).
    pub skipped_files: Vec<String>,
    /// Files left out only because the diff was too large.
    pub truncated: bool,
    /// Model name, id and provider.
    pub model: String,
    /// Provider's model id.
    pub model_id: String,
    /// Token usage.
    pub usage: Usage,
    /// Cost in US dollars.
    pub cost_usd: f64,
}

impl ReviewResult {
    /// pass / warn / fail for the check run.
    pub fn conclusion(&self) -> CheckConclusion {
        match self.findings.iter().map(|f| f.severity).max() {
            Some(Severity::Critical | Severity::High) => CheckConclusion::Failure,
            Some(Severity::Medium | Severity::Low) => CheckConclusion::Neutral,
            Some(Severity::Info) | None => CheckConclusion::Success,
        }
    }
}

fn ignore_set(config: &RepoConfig) -> GlobSet {
    let mut b = GlobSetBuilder::new();
    for p in DEFAULT_IGNORES
        .iter()
        .copied()
        .chain(config.review.ignore_paths.iter().map(String::as_str))
    {
        if let Ok(g) = Glob::new(p) {
            b.add(g);
        }
        // Also match at the repository root for "**/x" patterns.
        if let Some(rest) = p.strip_prefix("**/")
            && let Ok(g) = Glob::new(rest)
        {
            b.add(g);
        }
    }
    b.build().unwrap_or_else(|_| GlobSet::empty())
}

/// Run a review.
pub async fn review_diff(
    router: &Router,
    input: &ReviewInput,
    ctx: &CallContext,
) -> Result<ReviewResult, ReviewError> {
    let files = diff::parse(&input.diff);
    let ignored = ignore_set(&input.config);
    let marker = prompt::new_marker();

    let mut skipped = Vec::new();
    let mut reviewable: Vec<&FileDiff> = Vec::new();
    for f in &files {
        let path = f.path();
        if f.binary || f.new_path.is_none() || f.hunks.is_empty() || ignored.is_match(path) {
            skipped.push(path.to_owned());
        } else {
            reviewable.push(f);
        }
    }

    // Secrets in added lines become findings directly; the model never sees them.
    let mut secret_findings = Vec::new();
    for f in &reviewable {
        for line in f.added_lines() {
            if let Some(m) = f
                .line_text(line)
                .and_then(|t| secrets::scan(t).into_iter().next())
            {
                secret_findings.push(secret_finding(f.path(), line, m.kind));
            }
        }
    }

    // Build the prompt within budget.
    let mut user = String::new();
    user.push_str(&prompt::data_block(
        &marker,
        "Pull request title",
        &secrets::redact(&input.title),
    ));
    if !input.description.trim().is_empty() {
        user.push_str(&prompt::data_block(
            &marker,
            "Pull request description",
            &secrets::redact(&input.description),
        ));
    }
    if !input.rules.is_empty() {
        user.push_str("\nTeam rules (from the default branch):\n");
        for (name, text) in &input.rules {
            user.push_str(&format!("\n## {name}\n{}\n", secrets::redact(text)));
        }
    }
    let mut used = 0;
    let mut truncated = false;
    let mut in_prompt: Vec<&FileDiff> = Vec::new();
    for f in &reviewable {
        let rendered = f.render_numbered();
        if used + rendered.len() > MAX_DIFF_CHARS {
            truncated = true;
            skipped.push(f.path().to_owned());
            continue;
        }
        used += rendered.len();
        user.push_str(&prompt::data_block(
            &marker,
            &format!(
                "\nDiff of `{}` (new-file line numbers on the left)",
                f.path()
            ),
            &secrets::redact(&rendered),
        ));
        in_prompt.push(f);
    }
    if in_prompt.is_empty() {
        return Ok(empty_result(skipped, truncated, secret_findings));
    }
    let mut ctx_used = 0;
    for f in &in_prompt {
        if let Some(content) = input.files.get(f.path()) {
            if ctx_used + content.len() > MAX_CONTEXT_CHARS {
                continue;
            }
            ctx_used += content.len();
            let numbered: String = content
                .lines()
                .enumerate()
                .map(|(i, l)| format!("{:>5}  {l}\n", i + 1))
                .collect();
            user.push_str(&prompt::data_block(
                &marker,
                &format!("\nFull new version of `{}`, for context only", f.path()),
                &secrets::redact(&numbered),
            ));
        }
    }
    user.push_str("\nReview the diff above and call report_findings once.");

    let req = CompletionRequest {
        system: Some(prompt::system_prompt(&marker)),
        messages: vec![Message::user(user)],
        tools: vec![prompt::tool()],
        max_tokens: 4096,
        temperature: Some(0.0),
    };
    let routed = router.complete(Task::Review, &req, ctx).await?;

    let raw_items = extract_items(&routed.response)?;
    let by_path: BTreeMap<&str, &FileDiff> = in_prompt.iter().map(|f| (f.path(), *f)).collect();
    let mut out = ReviewResult {
        findings: Vec::new(),
        filtered_out: 0,
        off_diff: 0,
        malformed: 0,
        skipped_files: skipped,
        truncated,
        model: routed.model,
        model_id: routed.model_id,
        usage: routed.response.usage,
        cost_usd: routed.cost_usd,
    };
    let allow_suggestions = input.config.mode >= fixloop_core::domain::RepoMode::Suggest;
    let mut seen = BTreeSet::new();
    for item in raw_items {
        let Some(mut f) = parse_item(&item) else {
            out.malformed += 1;
            continue;
        };
        let Some(file) = by_path.get(f.path.as_str()) else {
            out.off_diff += 1;
            continue;
        };
        if !place_on_diff(&mut f, file) {
            out.off_diff += 1;
            continue;
        }
        if f.confidence < input.config.review.confidence_threshold
            || f.severity < input.config.review.min_severity
        {
            out.filtered_out += 1;
            continue;
        }
        if !allow_suggestions {
            f.suggestion = None;
        }
        if let (Some(sug), Some(orig)) = (&f.suggestion, file.line_text(f.line)) {
            f.suggestion = Some(match_indent(sug, orig));
        }
        f.fingerprint = fingerprint(&f, file);
        if seen.insert((f.path.clone(), f.line, f.title.to_lowercase())) {
            out.findings.push(f);
        }
    }
    // Secret findings win over a model finding on the same line.
    for s in secret_findings {
        out.findings
            .retain(|f| !(f.path == s.path && f.line == s.line));
        out.findings.push(s);
    }
    out.findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(b.confidence.total_cmp(&a.confidence))
            .then(a.path.cmp(&b.path))
            .then(a.line.cmp(&b.line))
    });
    out.findings.truncate(MAX_FINDINGS);
    Ok(out)
}

fn empty_result(skipped: Vec<String>, truncated: bool, secrets: Vec<Finding>) -> ReviewResult {
    ReviewResult {
        findings: secrets,
        filtered_out: 0,
        off_diff: 0,
        malformed: 0,
        skipped_files: skipped,
        truncated,
        model: String::new(),
        model_id: String::new(),
        usage: Usage::default(),
        cost_usd: 0.0,
    }
}

/// Pull the findings array from the tool call, or from JSON in the text.
fn extract_items(resp: &fixloop_llm::CompletionResponse) -> Result<Vec<Value>, ReviewError> {
    let args = resp
        .tool_calls()
        .into_iter()
        .find(|c| c.name == prompt::TOOL_NAME)
        .map(|c| c.arguments.clone())
        .or_else(|| extract_json_object(&resp.text()).map(Value::Object))
        .ok_or_else(|| {
            ReviewError::NoStructuredOutput("the model did not call report_findings".into())
        })?;
    match args.get("findings") {
        Some(Value::Array(items)) => Ok(items.clone()),
        // Some models send the array as a JSON string.
        Some(Value::String(s)) => serde_json::from_str::<Vec<Value>>(s)
            .map_err(|e| ReviewError::NoStructuredOutput(format!("findings is not a list: {e}"))),
        _ => Err(ReviewError::NoStructuredOutput(
            "report_findings was called without a findings list".into(),
        )),
    }
}

fn parse_item(v: &Value) -> Option<Finding> {
    let s = |k: &str| {
        v.get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let n = |k: &str| {
        v.get(k).and_then(|x| {
            x.as_u64()
                .or_else(|| x.as_f64().map(|f| f as u64))
                .or_else(|| x.as_str().and_then(|s| s.trim().parse().ok()))
        })
    };
    let severity = match s("severity")?.to_ascii_lowercase().as_str() {
        "critical" => Severity::Critical,
        "high" => Severity::High,
        "medium" => Severity::Medium,
        "low" => Severity::Low,
        "info" => Severity::Info,
        _ => return None,
    };
    let category = match s("category")?
        .to_ascii_lowercase()
        .replace('_', "-")
        .as_str()
    {
        "bug" => Category::Bug,
        "security" => Category::Security,
        "performance" => Category::Performance,
        "style" => Category::Style,
        "test-gap" | "testgap" | "test" => Category::TestGap,
        _ => return None,
    };
    let confidence = v
        .get("confidence")
        .and_then(|c| {
            c.as_f64()
                .or_else(|| c.as_str().and_then(|s| s.parse().ok()))
        })?
        .clamp(0.0, 1.0) as f32;
    let line = u32::try_from(n("line")?).ok().filter(|l| *l > 0)?;
    let path = s("path")?
        .trim_start_matches("./")
        .trim_start_matches("b/")
        .to_owned();
    Some(Finding {
        title: clip(&sanitize(s("title")?), 120),
        severity,
        category,
        path,
        line,
        end_line: n("end_line").and_then(|e| u32::try_from(e).ok()),
        rationale: clip(&sanitize(s("rationale")?), 2000),
        suggested_fix: s("suggested_fix").map(|t| clip(&sanitize(t), 1000)),
        // Suggestions are code: keep it verbatim apart from secrets, and
        // refuse anything that would break out of the suggestion block.
        suggestion: v
            .get("suggestion")
            .and_then(Value::as_str)
            .filter(|t| !t.contains("```") && t.len() <= 8000)
            .map(|t| secrets::redact(t.trim_end_matches('\n'))),
        confidence,
        fingerprint: String::new(),
    })
}

/// Make model text safe to post: no secrets, no @-mentions, no commands.
pub fn sanitize(text: &str) -> String {
    let t = secrets::redact(text);
    let mut out = String::with_capacity(t.len());
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '@' && chars.peek().is_some_and(|n| n.is_ascii_alphanumeric()) {
            out.push('\u{200B}');
        }
    }
    out.replace("/fixloop", "/\u{200B}fixloop")
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut c: String = s.chars().take(max).collect();
    c.push('…');
    c
}

/// Snap a finding onto a commentable line. Returns false if impossible.
fn place_on_diff(f: &mut Finding, file: &FileDiff) -> bool {
    let commentable = file.commentable_lines();
    if !commentable.contains(&f.line) {
        // Allow small numbering slips onto a nearby added line.
        let added = file.added_lines();
        let near = added
            .iter()
            .copied()
            .filter(|l| l.abs_diff(f.line) <= 2)
            .min_by_key(|l| l.abs_diff(f.line));
        match near {
            Some(l) => {
                f.line = l;
                f.end_line = None;
                f.suggestion = None; // the replacement no longer lines up
            }
            None => return false,
        }
    }
    if let Some(end) = f.end_line {
        let same_hunk = file.hunk_of(f.line) == file.hunk_of(end);
        let all_in = (f.line..=end).all(|l| commentable.contains(&l));
        if end <= f.line || !same_hunk || !all_in {
            f.end_line = None;
            if end != f.line {
                f.suggestion = None;
            }
        }
    }
    true
}

/// Models often drop leading whitespace from suggestions. If the whole
/// suggestion is unindented but the original line is indented, indent it
/// to match, so applying it on GitHub keeps the code's formatting.
fn match_indent(suggestion: &str, original_first_line: &str) -> String {
    let indent: String = original_first_line
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    let min_indent = suggestion
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    if indent.is_empty() || min_indent > 0 {
        return suggestion.to_owned();
    }
    suggestion
        .lines()
        .map(|l| {
            if l.trim().is_empty() {
                l.to_owned()
            } else {
                format!("{indent}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn fingerprint(f: &Finding, file: &FileDiff) -> String {
    let anchor = file
        .line_text(f.line)
        .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| f.title.to_lowercase());
    let mut h = Sha256::new();
    h.update(f.category.as_str());
    h.update([0]);
    h.update(&f.path);
    h.update([0]);
    h.update(anchor);
    hex::encode(&h.finalize()[..16])
}

fn secret_finding(path: &str, line: u32, kind: &str) -> Finding {
    let mut h = Sha256::new();
    h.update(format!("secret\0{path}\0{line}\0{kind}"));
    Finding {
        title: format!("Possible secret committed ({kind})"),
        severity: Severity::Critical,
        category: Category::Security,
        path: path.to_owned(),
        line,
        end_line: None,
        rationale: "This line appears to contain a credential. Anyone with read access to the repository or its history can use it. Fixloop did not send this value to any model.".into(),
        suggested_fix: Some("Remove it from the code, load it from the environment or a secret manager, and revoke and rotate the credential: it stays in git history even after removal.".into()),
        suggestion: None,
        confidence: 0.95,
        fingerprint: hex::encode(&h.finalize()[..16]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_neutralises_mentions_commands_and_secrets() {
        let s = sanitize("ping @alice, run /fixloop fix, key AKIAIOSFODNN7EXAMPLE, email a@b");
        assert!(!s.contains("@alice"));
        assert!(!s.contains("/fixloop"));
        assert!(!s.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(s.contains("@\u{200B}alice"));
    }

    #[test]
    fn suggestions_keep_original_indentation() {
        assert_eq!(
            match_indent("items[i - 1]", "    items[i]"),
            "    items[i - 1]"
        );
        assert_eq!(
            match_indent("if x {\n    y\n}", "\tif x {"),
            "\tif x {\n\t    y\n\t}"
        );
        // Already indented: left alone.
        assert_eq!(match_indent("    a", "    b"), "    a");
        assert_eq!(match_indent("a", "b"), "a");
    }

    #[test]
    fn parse_item_is_lenient_but_strict_on_meaning() {
        let ok = serde_json::json!({"title": "t", "severity": "HIGH", "category": "test_gap",
            "path": "./src/a.rs", "line": "7", "rationale": "r", "confidence": 1.5});
        let f = parse_item(&ok).unwrap();
        assert_eq!(
            (f.severity, f.category, f.line, f.path.as_str()),
            (Severity::High, Category::TestGap, 7, "src/a.rs")
        );
        assert!((f.confidence - 1.0).abs() < f32::EPSILON);
        let bad = serde_json::json!({"title": "t", "severity": "urgent", "category": "bug",
            "path": "a", "line": 1, "rationale": "r", "confidence": 0.9});
        assert!(parse_item(&bad).is_none());
        let fence = serde_json::json!({"title": "t", "severity": "low", "category": "bug",
            "path": "a", "line": 1, "rationale": "r", "confidence": 0.9, "suggestion": "x```y"});
        assert!(parse_item(&fence).unwrap().suggestion.is_none());
    }
}
