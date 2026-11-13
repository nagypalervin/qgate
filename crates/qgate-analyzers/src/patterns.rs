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
//!
//! [`NamespacePatterns`] are the same globs over `.`-separated C# names
//! (design D4 of `add-csharp-architecture`): `Shop.Infrastructure` also
//! matches `Shop.Infrastructure.Data`, and `**.Data` any name ending in a
//! `Data` segment. A namespace pattern with an empty segment, a `/`,
//! whitespace or an `@` is invalid.

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

/// A compiled list of C# namespace patterns: a name matches when any
/// pattern does. Each `.` is a segment separator, so the patterns follow
/// the rules of [`PathPatterns`] with `.` in place of `/`.
#[derive(Debug, Clone)]
pub struct NamespacePatterns {
    paths: PathPatterns,
}

impl NamespacePatterns {
    /// Compile `patterns`, as written.
    ///
    /// # Errors
    ///
    /// Returns the first pattern, as written, that is empty, has an empty
    /// segment, a `/`, whitespace or an `@`, or is not a valid glob.
    pub fn new(patterns: &[String]) -> Result<Self, InvalidPattern> {
        let mut as_paths = Vec::with_capacity(patterns.len());
        for written in patterns {
            check_namespace(written).map_err(|detail| InvalidPattern {
                pattern: written.clone(),
                detail: detail.to_owned(),
            })?;
            as_paths.push(written.replace('.', "/"));
        }
        // Checked first: path spelling would drop the empty segments.
        let paths = PathPatterns::new(&as_paths).map_err(|e| {
            let pattern = patterns
                .iter()
                .find(|written| written.replace('.', "/") == e.pattern)
                .cloned()
                .unwrap_or_else(|| patterns.join(", "));
            InvalidPattern {
                pattern,
                detail: e.detail,
            }
        })?;
        Ok(Self { paths })
    }

    /// Whether the `.`-separated `name` matches any of the patterns.
    #[must_use]
    pub fn matches(&self, name: &str) -> bool {
        self.paths.matches(&name.replace('.', "/"))
    }
}

/// The spelling rules of a namespace pattern.
fn check_namespace(pattern: &str) -> Result<(), &'static str> {
    if pattern.is_empty() {
        return Err("the pattern is empty");
    }
    if pattern.contains('/') {
        return Err("namespace patterns are `.`-separated; `/` is not allowed");
    }
    // Names are compared without whitespace and without the `@` of
    // verbatim identifiers, so a pattern with either could never match.
    if pattern.contains(char::is_whitespace) {
        return Err("a namespace pattern cannot contain whitespace");
    }
    if pattern.contains('@') {
        return Err("write names without `@`: `using @class.Foo;` uses `class.Foo`");
    }
    if pattern.split('.').any(str::is_empty) {
        return Err(
            "a namespace pattern cannot have an empty segment (`..`, or a leading or trailing `.`)",
        );
    }
    Ok(())
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

    fn namespaces(written: &[&str]) -> NamespacePatterns {
        let written: Vec<String> = written.iter().map(|&p| p.to_owned()).collect();
        NamespacePatterns::new(&written).expect("valid patterns")
    }

    fn invalid_namespace(written: &str) -> InvalidPattern {
        NamespacePatterns::new(&[written.to_owned()]).expect_err("invalid pattern")
    }

    // architecture-analysis: "A namespace and the names below it"
    #[test]
    fn a_namespace_matches_itself_and_below() {
        let p = namespaces(&["Shop.Infrastructure"]);
        assert!(p.matches("Shop.Infrastructure"));
        assert!(p.matches("Shop.Infrastructure.Data.Db"));
        assert!(!p.matches("Shop.InfrastructureTests"));
        assert!(!p.matches("Shop"));
        assert!(!p.matches("Other.Shop.Infrastructure"));
    }

    // architecture-analysis: "Any namespace ending in a segment"
    #[test]
    fn namespace_globs() {
        let p = namespaces(&["**.Data"]);
        assert!(p.matches("Shop.Data"));
        assert!(p.matches("Shop.Orders.Data"));
        assert!(!p.matches("Shop.Data.Rows"));
        assert!(!p.matches("Shop.DataAccess"));
        let p = namespaces(&["Shop.*.Data"]);
        assert!(p.matches("Shop.Orders.Data"));
        assert!(!p.matches("Shop.Orders.Sub.Data"));
        let p = namespaces(&["Shop.Infrastructure.**"]);
        assert!(p.matches("Shop.Infrastructure"));
        assert!(p.matches("Shop.Infrastructure.Data"));
        assert!(!p.matches("Shop.InfrastructureTests"));
        let p = namespaces(&[
            "Shop.{Infrastructure,Web}.**",
            "Microsoft.EntityFrameworkCore",
        ]);
        assert!(p.matches("Shop.Web.Api"));
        assert!(p.matches("Shop.Infrastructure"));
        assert!(p.matches("Microsoft.EntityFrameworkCore.Metadata"));
        assert!(!p.matches("Shop.Domain"));
        assert!(!namespaces(&[]).matches("Shop"));
    }

    // architecture-analysis: "Empty segment"
    #[test]
    fn namespace_spelling() {
        for written in ["Shop..Infrastructure", ".Shop", "Shop.", "."] {
            let error = invalid_namespace(written);
            assert_eq!(error.pattern, written);
            assert!(error.detail.contains("empty segment"), "{written}: {error}");
        }
        assert!(invalid_namespace("").detail.contains("empty"));
        let error = invalid_namespace("Shop/Infrastructure");
        assert_eq!(error.pattern, "Shop/Infrastructure");
        assert!(error.detail.contains('/'), "{error}");
        // A glob error names the pattern as written, not its path form,
        // also in a list.
        assert_eq!(invalid_namespace("Shop.[Infra").pattern, "Shop.[Infra");
        let list = ["Shop.Data".to_owned(), "Shop.[a".to_owned()];
        assert_eq!(
            NamespacePatterns::new(&list).expect_err("invalid").pattern,
            "Shop.[a"
        );
        // A typo that no normalized name could match is an error, not a
        // rule that silently never fires.
        for written in ["Shop.Infrastructure ", "Shop. Data", "Shop.@class"] {
            let error = invalid_namespace(written);
            assert_eq!(error.pattern, written);
            assert!(
                error.detail.contains("whitespace") || error.detail.contains('@'),
                "{written}: {error}"
            );
        }
    }
}
