//! Unified diff parsing.

use std::collections::BTreeSet;

/// Kind of diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Added (`+`).
    Added,
    /// Removed (`-`).
    Removed,
    /// Unchanged context.
    Context,
}

/// One line of a hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// Kind.
    pub kind: LineKind,
    /// Line number in the new file (none for removed lines).
    pub new_line: Option<u32>,
    /// Text without the leading marker.
    pub text: String,
}

/// A hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// The `@@ ... @@` header.
    pub header: String,
    /// Lines.
    pub lines: Vec<DiffLine>,
}

/// Changes to one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Path before the change (`None` for new files).
    pub old_path: Option<String>,
    /// Path after the change (`None` for deleted files).
    pub new_path: Option<String>,
    /// Binary file; no hunks.
    pub binary: bool,
    /// Hunks.
    pub hunks: Vec<Hunk>,
}

impl FileDiff {
    /// The path to show and comment on.
    pub fn path(&self) -> &str {
        self.new_path
            .as_deref()
            .or(self.old_path.as_deref())
            .unwrap_or_default()
    }

    /// New-file lines that were added.
    pub fn added_lines(&self) -> BTreeSet<u32> {
        self.lines_where(|k| k == LineKind::Added)
    }

    /// New-file lines that can carry an inline comment (added or context).
    pub fn commentable_lines(&self) -> BTreeSet<u32> {
        self.lines_where(|k| k != LineKind::Removed)
    }

    fn lines_where(&self, f: impl Fn(LineKind) -> bool) -> BTreeSet<u32> {
        self.hunks
            .iter()
            .flat_map(|h| &h.lines)
            .filter(|l| f(l.kind))
            .filter_map(|l| l.new_line)
            .collect()
    }

    /// Index of the hunk containing new-file `line`.
    pub fn hunk_of(&self, line: u32) -> Option<usize> {
        self.hunks.iter().position(|h| {
            h.lines
                .iter()
                .any(|l| l.kind != LineKind::Removed && l.new_line == Some(line))
        })
    }

    /// Text of new-file `line`, if it is in the diff.
    pub fn line_text(&self, line: u32) -> Option<&str> {
        self.hunks
            .iter()
            .flat_map(|h| &h.lines)
            .find(|l| l.kind != LineKind::Removed && l.new_line == Some(line))
            .map(|l| l.text.as_str())
    }

    /// Render the hunks with new-file line numbers, for the model:
    /// `  12 + added`, `     - removed`, `  13   context`.
    pub fn render_numbered(&self) -> String {
        let mut out = String::new();
        for h in &self.hunks {
            out.push_str(&h.header);
            out.push('\n');
            for l in &h.lines {
                let (num, mark) = match l.kind {
                    LineKind::Added => (l.new_line, '+'),
                    LineKind::Removed => (None, '-'),
                    LineKind::Context => (l.new_line, ' '),
                };
                match num {
                    Some(n) => out.push_str(&format!("{n:>5} {mark} ")),
                    None => out.push_str(&format!("      {mark} ")),
                }
                out.push_str(&l.text);
                out.push('\n');
            }
        }
        out
    }
}

fn strip_prefix_path(p: &str) -> Option<String> {
    let p = p.trim_end_matches('\t').trim();
    if p == "/dev/null" {
        return None;
    }
    let p = p
        .strip_prefix("a/")
        .or_else(|| p.strip_prefix("b/"))
        .unwrap_or(p);
    Some(p.to_owned())
}

fn parse_new_start(header: &str) -> Option<u32> {
    // @@ -a,b +c,d @@
    let plus = header.split_whitespace().find(|t| t.starts_with('+'))?;
    plus[1..].split(',').next()?.parse().ok()
}

/// Parse a git-style unified diff. Unknown lines are ignored.
pub fn parse(diff: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut new_line = 0u32;
    for raw in diff.lines() {
        if let Some(rest) = raw.strip_prefix("diff --git ") {
            // Paths from the header are a fallback; ---/+++ lines win.
            let mut parts = rest.splitn(2, " b/");
            let old = parts.next().map(|p| p.trim_start_matches("a/").to_owned());
            let new = parts.next().map(str::to_owned);
            files.push(FileDiff {
                old_path: old,
                new_path: new,
                binary: false,
                hunks: Vec::new(),
            });
            continue;
        }
        let Some(file) = files.last_mut() else {
            continue;
        };
        if let Some(h) = file.hunks.last_mut()
            && !raw.starts_with("@@")
            && !raw.starts_with("--- ")
            && !raw.starts_with("+++ ")
        {
            let (kind, text) = match raw.chars().next() {
                Some('+') => (LineKind::Added, &raw[1..]),
                Some('-') => (LineKind::Removed, &raw[1..]),
                Some(' ') => (LineKind::Context, &raw[1..]),
                Some('\\') => continue, // "\ No newline at end of file"
                None => (LineKind::Context, ""),
                _ => continue,
            };
            let num = if kind == LineKind::Removed {
                None
            } else {
                let n = new_line;
                new_line += 1;
                Some(n)
            };
            h.lines.push(DiffLine {
                kind,
                new_line: num,
                text: text.to_owned(),
            });
            continue;
        }
        if let Some(p) = raw.strip_prefix("--- ") {
            file.old_path = strip_prefix_path(p);
        } else if let Some(p) = raw.strip_prefix("+++ ") {
            file.new_path = strip_prefix_path(p);
        } else if raw.starts_with("deleted file mode") {
            file.new_path = None;
        } else if raw.starts_with("new file mode") {
            file.old_path = None;
        } else if raw.starts_with("Binary files ") || raw.starts_with("GIT binary patch") {
            file.binary = true;
        } else if raw.starts_with("@@") {
            new_line = parse_new_start(raw).unwrap_or(1);
            file.hunks.push(Hunk {
                header: raw.to_owned(),
                lines: Vec::new(),
            });
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/src/util.rs b/src/util.rs
index 1111111..2222222 100644
--- a/src/util.rs
+++ b/src/util.rs
@@ -1,3 +1,8 @@
 pub fn first(items: &[i32]) -> Option<i32> {
     items.first().copied()
 }
+
+pub fn last(items: &[i32]) -> i32 {
+    items[items.len()]
+}
diff --git a/old.txt b/old.txt
deleted file mode 100644
--- a/old.txt
+++ /dev/null
@@ -1 +0,0 @@
-gone
diff --git a/img.png b/img.png
new file mode 100644
Binary files /dev/null and b/img.png differ
diff --git a/a.py b/a.py
--- a/a.py
+++ b/a.py
@@ -10,3 +10,3 @@ def f():
     x = 1
-    y = 2
+    y = 3
     return x
\\ No newline at end of file
";

    #[test]
    fn parses_files_lines_and_numbers() {
        let files = parse(DIFF);
        assert_eq!(files.len(), 4);
        let util = &files[0];
        assert_eq!(util.path(), "src/util.rs");
        assert_eq!(
            util.added_lines().into_iter().collect::<Vec<_>>(),
            vec![4, 5, 6, 7]
        );
        assert_eq!(util.line_text(6), Some("    items[items.len()]"));
        assert_eq!(util.commentable_lines().len(), 7);

        assert_eq!(files[1].new_path, None);
        assert_eq!(files[1].path(), "old.txt");
        assert!(files[2].binary);
        assert_eq!(files[2].old_path, None);

        let py = &files[3];
        assert_eq!(py.added_lines().into_iter().collect::<Vec<_>>(), vec![11]);
        assert_eq!(
            py.commentable_lines().into_iter().collect::<Vec<_>>(),
            vec![10, 11, 12]
        );
    }

    #[test]
    fn renders_with_line_numbers() {
        let r = parse(DIFF)[3].render_numbered();
        assert!(r.contains("   11 +     y = 3"), "{r}");
        assert!(r.contains("      -     y = 2"), "{r}");
        assert!(r.contains("   10       x = 1"), "{r}");
    }
}
