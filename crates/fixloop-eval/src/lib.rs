//! Evaluation harness: score how well a model finds and fixes known bugs.
//!
//! An eval **case** is a diff with known bugs (`eval/cases/*.yaml`). A run
//! produces **predictions** (findings with a path and line). Scoring matches
//! predictions to expected bugs and reports found bugs, false alarms and
//! missed bugs, so frontier and local models can be compared on the same
//! cases. Fix pass rate is recorded once the Fixer exists (Phase 4).
//!
//! Running a model over the cases needs the Reviewer (Phase 2); this crate
//! provides the case format, loading and scoring.

use std::path::{Path, PathBuf};

use fixloop_core::domain::Category;
use serde::{Deserialize, Serialize};

/// How far (in lines) a prediction may be from an expected bug and still match.
pub const LINE_TOLERANCE: u32 = 3;

/// A known bug in a case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedFinding {
    /// File path, as in the diff.
    pub path: String,
    /// First line of the bug (new-file line numbers).
    pub line_start: u32,
    /// Last line of the bug.
    pub line_end: u32,
    /// Kind of problem.
    pub category: Category,
    /// What the bug is, for humans reading results.
    pub description: String,
}

/// One evaluation case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalCase {
    /// Unique id (defaults to the file name).
    #[serde(default)]
    pub id: String,
    /// Programming language.
    pub language: String,
    /// What the change does.
    pub description: String,
    /// Unified diff under review.
    pub diff: String,
    /// Bugs a good reviewer should report. Empty means "clean change": any
    /// finding is a false alarm.
    #[serde(default)]
    pub expected: Vec<ExpectedFinding>,
}

/// A finding produced by the system under test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prediction {
    /// File path.
    pub path: String,
    /// Line the finding points at.
    pub line: u32,
    /// Kind of problem.
    pub category: Category,
}

/// Result of one case.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseScore {
    /// Expected bugs that were found.
    pub found: u32,
    /// Predictions that match no expected bug.
    pub false_alarms: u32,
    /// Expected bugs that were not found.
    pub missed: u32,
    /// Fixes attempted (Phase 4).
    pub fixes_attempted: u32,
    /// Fixes that passed build, tests and lint (Phase 4).
    pub fixes_passed: u32,
}

/// Errors loading cases.
#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    /// A case file could not be read.
    #[error("could not read `{path}`: {source}")]
    Read {
        /// File.
        path: PathBuf,
        /// Cause.
        #[source]
        source: std::io::Error,
    },
    /// A case file is invalid.
    #[error("invalid case `{path}`: {message}")]
    Invalid {
        /// File.
        path: PathBuf,
        /// What is wrong.
        message: String,
    },
}

/// Load every `*.yaml` case in `dir`, sorted by id.
pub fn load_cases(dir: &Path) -> Result<Vec<EvalCase>, EvalError> {
    let read_err = |source| EvalError::Read {
        path: dir.to_path_buf(),
        source,
    };
    let mut cases = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(read_err)? {
        let path = entry.map_err(read_err)?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|source| EvalError::Read {
            path: path.clone(),
            source,
        })?;
        let mut case: EvalCase = serde_saphyr::from_str(&text).map_err(|e| EvalError::Invalid {
            path: path.clone(),
            message: e.to_string(),
        })?;
        if case.id.is_empty() {
            case.id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_owned();
        }
        for e in &case.expected {
            if e.line_start > e.line_end {
                return Err(EvalError::Invalid {
                    path: path.clone(),
                    message: format!("{}: line_start is after line_end", e.path),
                });
            }
        }
        cases.push(case);
    }
    cases.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(cases)
}

/// Score predictions for one case. Each expected bug can be matched by at
/// most one prediction; a prediction matches if the path is equal and the
/// line is within [`LINE_TOLERANCE`] of the bug's range. Category is not
/// required to match (a real bug reported as "security" still counts).
pub fn score(case: &EvalCase, predictions: &[Prediction]) -> CaseScore {
    let mut matched = vec![false; case.expected.len()];
    let mut false_alarms = 0;
    for p in predictions {
        let hit = case.expected.iter().enumerate().find(|(i, e)| {
            !matched[*i]
                && e.path == p.path
                && p.line + LINE_TOLERANCE >= e.line_start
                && p.line <= e.line_end + LINE_TOLERANCE
        });
        match hit {
            Some((i, _)) => matched[i] = true,
            None => false_alarms += 1,
        }
    }
    let found = matched.iter().filter(|m| **m).count() as u32;
    CaseScore {
        found,
        false_alarms,
        missed: case.expected.len() as u32 - found,
        ..CaseScore::default()
    }
}

/// Totals across cases, with derived rates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// Summed counts.
    pub totals: CaseScore,
    /// found / (found + false alarms).
    pub precision: f64,
    /// found / (found + missed).
    pub recall: f64,
    /// fixes passed / fixes attempted.
    pub fix_pass_rate: f64,
}

/// Summarise case scores.
pub fn summarize(scores: &[CaseScore]) -> Summary {
    let mut t = CaseScore::default();
    for s in scores {
        t.found += s.found;
        t.false_alarms += s.false_alarms;
        t.missed += s.missed;
        t.fixes_attempted += s.fixes_attempted;
        t.fixes_passed += s.fixes_passed;
    }
    let ratio = |a: u32, b: u32| {
        if b == 0 {
            0.0
        } else {
            f64::from(a) / f64::from(b)
        }
    };
    Summary {
        totals: t,
        precision: ratio(t.found, t.found + t.false_alarms),
        recall: ratio(t.found, t.found + t.missed),
        fix_pass_rate: ratio(t.fixes_passed, t.fixes_attempted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case() -> EvalCase {
        EvalCase {
            id: "c".into(),
            language: "rust".into(),
            description: String::new(),
            diff: String::new(),
            expected: vec![
                ExpectedFinding {
                    path: "src/a.rs".into(),
                    line_start: 10,
                    line_end: 12,
                    category: Category::Bug,
                    description: String::new(),
                },
                ExpectedFinding {
                    path: "src/b.rs".into(),
                    line_start: 5,
                    line_end: 5,
                    category: Category::Security,
                    description: String::new(),
                },
            ],
        }
    }

    fn p(path: &str, line: u32) -> Prediction {
        Prediction {
            path: path.into(),
            line,
            category: Category::Bug,
        }
    }

    #[test]
    fn matches_within_tolerance_and_counts_misses() {
        let s = score(&case(), &[p("src/a.rs", 14), p("src/c.rs", 1)]);
        assert_eq!((s.found, s.false_alarms, s.missed), (1, 1, 1));
    }

    #[test]
    fn duplicate_predictions_count_as_false_alarms() {
        let s = score(&case(), &[p("src/b.rs", 5), p("src/b.rs", 6)]);
        assert_eq!((s.found, s.false_alarms), (1, 1));
    }

    #[test]
    fn too_far_is_a_miss() {
        let s = score(&case(), &[p("src/a.rs", 16)]);
        assert_eq!((s.found, s.false_alarms, s.missed), (0, 1, 2));
    }

    #[test]
    fn summary_rates() {
        let s = summarize(&[
            CaseScore {
                found: 3,
                false_alarms: 1,
                missed: 1,
                fixes_attempted: 2,
                fixes_passed: 1,
            },
            CaseScore::default(),
        ]);
        assert!((s.precision - 0.75).abs() < 1e-9);
        assert!((s.recall - 0.75).abs() < 1e-9);
        assert!((s.fix_pass_rate - 0.5).abs() < 1e-9);
    }

    #[test]
    fn bundled_cases_load() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../eval/cases");
        let cases = load_cases(&dir).unwrap();
        assert!(cases.len() >= 3, "found {}", cases.len());
        assert!(
            cases.iter().any(|c| c.expected.is_empty()),
            "need a clean control case"
        );
    }
}
