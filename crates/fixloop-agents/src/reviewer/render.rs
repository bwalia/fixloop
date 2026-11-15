//! Markdown for review comments, summaries and check runs.

use fixloop_core::domain::{Finding, Severity};

use super::review::ReviewResult;

/// Name of the check run.
pub const CHECK_NAME: &str = "Fixloop review";

fn badge(s: Severity) -> &'static str {
    match s {
        Severity::Critical => "🔴 critical",
        Severity::High => "🟠 high",
        Severity::Medium => "🟡 medium",
        Severity::Low => "🔵 low",
        Severity::Info => "⚪ info",
    }
}

/// Body of one inline comment.
pub fn comment_body(f: &Finding) -> String {
    let mut b = format!(
        "**{} · {}: {}**\n\n{}",
        badge(f.severity),
        f.category.as_str(),
        f.title,
        f.rationale
    );
    if let Some(fix) = &f.suggested_fix {
        b.push_str(&format!("\n\n**Fix:** {fix}"));
    }
    if let Some(s) = &f.suggestion {
        b.push_str(&format!("\n\n```suggestion\n{s}\n```"));
    }
    b.push_str(&format!(
        "\n\n<sub>Fixloop · confidence {:.0}%</sub>",
        f.confidence * 100.0
    ));
    b
}

fn counts(findings: &[Finding]) -> String {
    let mut parts = Vec::new();
    for s in [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
        Severity::Info,
    ] {
        let n = findings.iter().filter(|f| f.severity == s).count();
        if n > 0 {
            parts.push(format!("{n} {}", s.as_str()));
        }
    }
    if parts.is_empty() {
        "no problems found".into()
    } else {
        parts.join(", ")
    }
}

/// One-line check-run title.
pub fn check_title(r: &ReviewResult) -> String {
    let n = r.findings.len();
    if n == 0 {
        "No problems found".into()
    } else {
        format!(
            "{n} finding{}: {}",
            if n == 1 { "" } else { "s" },
            counts(&r.findings)
        )
    }
}

/// Review summary (also used as the check-run summary).
pub fn summary(r: &ReviewResult) -> String {
    let mut s = format!(
        "### Fixloop review\n\n{}.\n",
        capitalise(&counts(&r.findings))
    );
    if !r.findings.is_empty() {
        s.push_str("\n| Severity | Finding | Location |\n|---|---|---|\n");
        for f in &r.findings {
            s.push_str(&format!(
                "| {} | {} | `{}:{}` |\n",
                badge(f.severity),
                f.title.replace('|', "\\|"),
                f.path,
                f.line
            ));
        }
    }
    if !r.skipped_files.is_empty() {
        s.push_str(&format!(
            "\n{} not reviewed{}: {}\n",
            plural(r.skipped_files.len(), "file"),
            if r.truncated {
                " (the change is larger than one review covers)"
            } else {
                " (ignored, binary or deleted)"
            },
            r.skipped_files
                .iter()
                .take(20)
                .map(|p| format!("`{p}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let hidden = r.filtered_out + r.off_diff;
    if hidden > 0 {
        s.push_str(&format!(
            "\n{} below this repository's confidence or severity threshold, or outside the changed lines, {} not shown.\n",
            plural(hidden, "finding"),
            if hidden == 1 { "is" } else { "are" }
        ));
    }
    if !r.model.is_empty() {
        s.push_str(&format!(
            "\n<sub>Model: {} · {} tokens{} · comment <code>/fixloop review</code> to run again</sub>\n",
            r.model,
            r.usage.total(),
            if r.cost_usd > 0.0 { format!(" · ${:.4}", r.cost_usd) } else { String::new() }
        ));
    }
    s
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| {
        f.to_uppercase().collect::<String>() + c.as_str()
    })
}
