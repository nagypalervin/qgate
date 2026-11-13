//! Path patterns, shared by architecture rules (`from`, `to`) and the
//! file exclusion of `[files] exclude` (design D2 of `add-path-exclusion`).
//!
//! A pattern is a glob over `/`-separated paths, relative to the
//! repository root: `*` and `?` stay within one segment, `**` spans
//! segments, `[...]` is a character class. A pattern without glob
//! characters also matches everything below it (`react` matches
//! `react/jsx-runtime`), and `dir/**` also matches `dir`. Spelling rules:
//! leading `./` are dropped; a pattern that then starts with `/`, has a
//! `..` segment or is empty is invalid; empty and `.` segments and a
//! trailing `/` are ignored, except in a pattern with braces or a `\`
//! escape.

use std::fmt;

use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};

/// A compiled list of path patterns: a path matches when any pattern does.
#[derive(Debug, Clone)]
pub struct PathPatterns {
    set: GlobSet,
}

/// A pattern that is not valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidPattern {
    /// The pattern as written.
    pub pattern: String,
    /// What is wrong with it.
    pub detail: String,
}

impl fmt::Display for InvalidPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid pattern {:?}: {}", self.pattern, self.detail)
    }
}

impl std::error::Error for InvalidPattern {}

impl PathPatterns {
    /// Compile `patterns`, as written.
    ///
    /// # Errors
    ///
    /// Returns the first pattern that breaks the spelling rules or is not
    /// a valid glob.
    pub fn new(patterns: &[String]) -> Result<Self, InvalidPattern> {
        let mut set = GlobSetBuilder::new();
        for written in patterns {
            let error = |detail: String| InvalidPattern {
                pattern: written.clone(),
                detail,
            };
            let pattern = normalize(written).map_err(|detail| error(detail.to_owned()))?;
            let pattern = pattern.as_str();
            let has_glob = pattern.contains(['*', '?', '[', '{']);
            let mut globs = vec![pattern.to_owned()];
            if !has_glob {
                // `react` also matches `react/jsx-runtime`.
                globs.push(format!("{pattern}/**"));
            } else if let Some(dir) = pattern.strip_suffix("/**") {
                // `src/ui/**` also matches `src/ui`, the target of `import "../ui"`.
                globs.push(dir.to_owned());
            }
            for glob in globs {
                set.add(glob_of(&glob).map_err(|e| error(e.kind().to_string()))?);
            }
        }
        let set = set.build().map_err(|e| InvalidPattern {
            pattern: patterns.join(", "),
            detail: e.kind().to_string(),
        })?;
        Ok(Self { set })
    }

    /// Whether `path`, `/`-separated and relative to the repository root,
    /// matches any of the patterns.
    #[must_use]
    pub fn matches(&self, path: &str) -> bool {
        self.set.is_match(path)
    }
}

/// A pattern in the form paths have. Leading `./` are removed; a pattern
/// that then starts with `/`, or has a `..` segment anywhere (also inside
/// braces), is invalid, as is an empty one: none could ever match a path
/// from the repository root. A pattern with braces or a `\` escape keeps
/// its glob syntax and is not rewritten further; in any other pattern,
/// empty and `.` segments and a trailing `/` are dropped, so `./src//ui/`
/// means `src/ui`.
pub(crate) fn normalize(pattern: &str) -> Result<String, &'static str> {
    let mut rest = pattern;
    while let Some(after) = rest.strip_prefix("./") {
        rest = after;
    }
    if rest.starts_with('/') {
        return Err("patterns are relative to the repository root; remove the leading `/`");
    }
    if rest.split(['/', '{', '}', ',']).any(|part| part == "..") {
        return Err("`..` cannot match a path from the repository root");
    }
    // Braces and escapes keep their glob meaning: no further rewriting.
    if rest.contains(['{', '\\']) {
        return Ok(rest.to_owned());
    }
    let mut segments = Vec::new();
    for segment in rest.split('/') {
        match segment {
            "" | "." => {}
            segment => segments.push(segment),
        }
    }
    if segments.is_empty() {
        return Err("the pattern is empty");
    }
    Ok(segments.join("/"))
}

fn glob_of(pattern: &str) -> Result<Glob, globset::Error> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .backslash_escape(true)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patterns(written: &[&str]) -> PathPatterns {
        let written: Vec<String> = written.iter().map(|&p| p.to_owned()).collect();
        PathPatterns::new(&written).expect("valid patterns")
    }

    fn invalid(written: &str) -> InvalidPattern {
        PathPatterns::new(&[written.to_owned()]).expect_err("invalid pattern")
    }

    #[test]
    fn any_pattern_matches() {
        let p = patterns(&["**/Migrations/**", "vendor"]);
        assert!(p.matches("src/Data/Migrations/20261101_Init.cs"));
        assert!(p.matches("Migrations/x.cs"));
        assert!(p.matches("vendor/Dates.cs"));
        assert!(p.matches("vendor"));
        assert!(!p.matches("src/vendor/Dates.cs"));
        assert!(!p.matches("src/Migration/x.cs"));
    }

    #[test]
    fn no_patterns_match_nothing() {
        assert!(!patterns(&[]).matches("src/a.ts"));
    }

    #[test]
    fn spelling() {
        let p = patterns(&["./legacy//", "generated/**"]);
        assert!(p.matches("legacy/Report.cs"));
        assert!(p.matches("generated/Client.ts"));
        assert!(p.matches("generated"));
        assert_eq!(invalid("/src/**").pattern, "/src/**");
        assert!(invalid("/src/**").detail.contains("leading `/`"));
        assert!(invalid("src/../x").detail.contains("`..`"));
        assert!(invalid("./").detail.contains("empty"));
        assert_eq!(invalid("src/[a").pattern, "src/[a");
    }
}
