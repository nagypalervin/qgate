//! Forbidden imports between parts of a codebase.
//!
//! A [`ForbidRule`] says that files whose path matches `from` must not
//! import a target that matches `to`. The analyzer reports a violation only
//! if the change introduced it: the file's base version (at its old path,
//! if it was renamed) did not already import the same target against the
//! same rule. Rules are plain values; reading them from `qgate.toml` is the
//! caller's job (ADR 0003).
//!
//! Import targets (design D7 of `add-forbidden-imports`):
//!
//! - a specifier starting with `./` or `../` is resolved against the
//!   importing file's directory into a path from the repository root;
//!   one that leaves the root has no target;
//! - any other specifier (`react`, `@/ui/Button`) is its own target.
//!
//! Patterns are globs over `/`-separated paths: `*` and `?` stay within one
//! segment, `**` spans segments, `[...]` is a character class. A pattern
//! without glob characters also matches everything below it (`react`
//! matches `react/jsx-runtime`), and `dir/**` also matches `dir`.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use qgate_core::{Finding, Location, Severity};
use qgate_lang::FileModel;

use crate::{Analyzer, FileChange};

/// Rule id of a newly introduced forbidden import.
pub const RULE: &str = "architecture/forbidden-import";

/// One forbid rule, as plain values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForbidRule {
    /// Unique name, shown in findings.
    pub name: String,
    /// Patterns for the paths of importing files.
    pub from: Vec<String>,
    /// Patterns for import targets.
    pub to: Vec<String>,
}

/// A pattern that is not a valid glob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternError {
    /// The rule the pattern belongs to.
    pub rule: String,
    /// The pattern as written.
    pub pattern: String,
    /// What is wrong with it.
    pub detail: String,
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rule {:?}: invalid pattern {:?}: {}",
            self.rule, self.pattern, self.detail
        )
    }
}

impl std::error::Error for PatternError {}

/// Reports imports that a change newly introduces against a forbid rule.
#[derive(Debug, Clone)]
pub struct ArchitectureAnalyzer {
    rules: Vec<CompiledRule>,
}

#[derive(Debug, Clone)]
struct CompiledRule {
    name: String,
    from: GlobSet,
    to: GlobSet,
}

impl ArchitectureAnalyzer {
    /// Compile `rules`.
    ///
    /// # Errors
    ///
    /// A [`PatternError`] for the first pattern that is not a valid glob.
    pub fn new(rules: &[ForbidRule]) -> Result<Self, PatternError> {
        let rules = rules
            .iter()
            .map(|rule| {
                Ok(CompiledRule {
                    name: rule.name.clone(),
                    from: compile(&rule.name, &rule.from)?,
                    to: compile(&rule.name, &rule.to)?,
                })
            })
            .collect::<Result<_, PatternError>>()?;
        Ok(Self { rules })
    }

    /// Whether there are no rules, so the analyzer can never report.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// The violations of one version of a file: for each (rule index,
    /// target), the line and specifier of the first import of the target.
    fn violations(
        &self,
        path: &str,
        model: &FileModel,
    ) -> BTreeMap<(usize, String), (u32, String)> {
        let mut found = BTreeMap::new();
        for (index, rule) in self.rules.iter().enumerate() {
            if !rule.from.is_match(path) {
                continue;
            }
            for import in &model.imports {
                let Some(target) = target(path, &import.specifier) else {
                    continue;
                };
                if rule.to.is_match(&target) {
                    found
                        .entry((index, target))
                        .or_insert_with(|| (import.line, import.specifier.clone()));
                }
            }
        }
        found
    }
}

impl Analyzer for ArchitectureAnalyzer {
    fn name(&self) -> &'static str {
        "architecture"
    }

    fn analyze(&self, change: &FileChange) -> Vec<Finding> {
        let path = slash_path(&change.path);
        let old_path = change
            .old_path
            .as_deref()
            .map_or_else(|| path.clone(), slash_path);
        let base = self.violations(&old_path, &change.before);
        self.violations(&path, &change.after)
            .into_iter()
            .filter(|(key, _)| !base.contains_key(key))
            .map(|((index, target), (line, specifier))| {
                let shown = if specifier == target {
                    target
                } else {
                    format!("{target} (\"{specifier}\")")
                };
                Finding {
                    rule: RULE.to_owned(),
                    severity: Severity::Error,
                    message: format!(
                        "imports {shown}, forbidden by \"{}\"",
                        self.rules[index].name
                    ),
                    location: Location {
                        path: change.path.clone(),
                        line: Some(line),
                    },
                    metric: None,
                }
            })
            .collect()
    }
}

/// A path as git writes it, with `/` separators.
fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// The target of `specifier` imported by the file at `importer` (design
/// D7): a relative specifier resolved from the importer's directory, or
/// the specifier itself. `None` if a relative specifier leaves the root.
fn target(importer: &str, specifier: &str) -> Option<String> {
    let relative = specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../");
    if !relative {
        return Some(specifier.to_owned());
    }
    let mut segments: Vec<&str> = importer.split('/').collect();
    segments.pop(); // the file name
    for segment in specifier.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            name => segments.push(name),
        }
    }
    let path = segments.join("/");
    (!path.is_empty()).then_some(path)
}

/// Compile the patterns of one side of a rule into a set (design D3).
fn compile(rule: &str, patterns: &[String]) -> Result<GlobSet, PatternError> {
    let mut set = GlobSetBuilder::new();
    for pattern in patterns {
        let error = |detail: String| PatternError {
            rule: rule.to_owned(),
            pattern: pattern.clone(),
            detail,
        };
        let has_glob = pattern.contains(['*', '?', '[', '{']);
        let mut globs = vec![pattern.clone()];
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
    set.build().map_err(|e| PatternError {
        rule: rule.to_owned(),
        pattern: patterns.join(", "),
        detail: e.kind().to_string(),
    })
}

fn glob_of(pattern: &str) -> Result<Glob, globset::Error> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .backslash_escape(true)
        .build()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use qgate_lang::{Language, parse};

    use super::*;

    fn rule(name: &str, from: &[&str], to: &[&str]) -> ForbidRule {
        ForbidRule {
            name: name.to_owned(),
            from: from.iter().map(|&s| s.to_owned()).collect(),
            to: to.iter().map(|&s| s.to_owned()).collect(),
        }
    }

    fn domain_is_pure() -> ForbidRule {
        rule("domain-is-pure", &["src/domain/**"], &["src/ui/**"])
    }

    /// Run the analyzer with `rules` on a change of `path` (renamed from
    /// `old_path`, if given) and return `(line, message)` pairs.
    fn run(
        rules: &[ForbidRule],
        old_path: Option<&str>,
        path: &str,
        before: &str,
        after: &str,
    ) -> Vec<(u32, String)> {
        let change = FileChange {
            path: PathBuf::from(path),
            old_path: old_path.map(PathBuf::from),
            before: parse(Language::TypeScript, before).expect("before parses"),
            after: parse(Language::TypeScript, after).expect("after parses"),
        };
        ArchitectureAnalyzer::new(rules)
            .expect("valid rules")
            .analyze(&change)
            .into_iter()
            .map(|f| {
                assert_eq!(f.rule, RULE);
                assert_eq!(f.severity, Severity::Error);
                assert_eq!(f.location.path, PathBuf::from(path));
                (f.location.line.expect("a line"), f.message)
            })
            .collect()
    }

    macro_rules! fixture {
        ($rules:expr, $old:expr, $path:expr, $case:literal) => {
            run(
                $rules,
                $old,
                $path,
                include_str!(concat!("../fixtures/architecture/", $case, "/before.ts")),
                include_str!(concat!("../fixtures/architecture/", $case, "/after.ts")),
            )
        };
    }

    fn finding(line: u32, message: &str) -> (u32, String) {
        (line, message.to_owned())
    }

    // Requirement: Forbid rules

    #[test]
    fn domain_importing_the_ui() {
        assert_eq!(
            fixture!(
                &[domain_is_pure()],
                None,
                "src/domain/order.ts",
                "new_import"
            ),
            [finding(
                3,
                "imports src/ui/Button (\"../ui/Button\"), forbidden by \"domain-is-pure\""
            )]
        );
    }

    #[test]
    fn file_outside_from() {
        assert_eq!(
            fixture!(&[domain_is_pure()], None, "src/app/page.ts", "new_import"),
            []
        );
    }

    // Requirement: Imports that count (through the analyzer)

    #[test]
    fn every_import_form() {
        let targets: Vec<String> =
            fixture!(&[domain_is_pure()], None, "src/domain/a.ts", "every_form")
                .into_iter()
                .map(|(_, m)| m.split(' ').nth(1).unwrap_or_default().to_owned())
                .collect();
        assert_eq!(
            targets,
            [
                "src/ui/all",
                "src/ui/b",
                "src/ui/d",
                "src/ui/j.json",
                "src/ui/m",
                "src/ui/r",
                "src/ui/style",
                "src/ui/t",
            ]
        );
    }

    // Requirement: Import targets

    #[test]
    fn relative_targets_are_resolved_and_normalized() {
        assert_eq!(
            target("src/domain/order.ts", "./../ui/./Button").as_deref(),
            Some("src/ui/Button")
        );
        assert_eq!(target("src/a.ts", "./b").as_deref(), Some("src/b"));
        // The repository root itself is no target.
        assert_eq!(target("src/a.ts", ".."), None);
        assert_eq!(target("src/domain/a.ts", "..").as_deref(), Some("src"));
        assert_eq!(
            target("src/domain/a.ts", "../ui").as_deref(),
            Some("src/ui")
        );
        assert_eq!(target("a.ts", "./x//y").as_deref(), Some("x/y"));
    }

    #[test]
    fn leaving_the_repository_has_no_target() {
        assert_eq!(target("src/a.ts", "../../outside"), None);
        assert_eq!(target("a.ts", "../x"), None);
    }

    #[test]
    fn packages_and_aliases_are_their_own_target() {
        for specifier in [
            "react/jsx-runtime",
            "@/ui/Button",
            "@prisma/client",
            "src/ui/x",
        ] {
            assert_eq!(target("src/a.ts", specifier).as_deref(), Some(specifier));
        }
    }

    // Requirement: Patterns

    fn matches(pattern: &str, path: &str) -> bool {
        compile("r", &[pattern.to_owned()])
            .expect("valid pattern")
            .is_match(path)
    }

    #[test]
    fn package_name_matches_itself_and_below() {
        assert!(matches("react", "react"));
        assert!(matches("react", "react/jsx-runtime"));
        assert!(!matches("react", "react-dom"));
        assert!(matches("@prisma/client", "@prisma/client"));
    }

    #[test]
    fn directory_glob_matches_the_directory_itself() {
        assert!(matches("src/ui/**", "src/ui"));
        assert!(matches("src/ui/**", "src/ui/Button"));
        assert!(matches("src/ui/**", "src/ui/forms/Input"));
        assert!(!matches("src/ui/**", "src/uikit/Button"));
    }

    #[test]
    fn single_segment_wildcards() {
        assert!(matches("src/*/model.ts", "src/order/model.ts"));
        assert!(!matches("src/*/model.ts", "src/order/sub/model.ts"));
        assert!(matches("src/?/x", "src/a/x"));
        assert!(!matches("src/?/x", "src/ab/x"));
        assert!(matches("src/[ab]/x", "src/b/x"));
    }

    #[test]
    fn alias_pattern() {
        let rules = [rule("no-ui", &["src/domain/**"], &["@/ui/**"])];
        assert_eq!(
            run(
                &rules,
                None,
                "src/domain/a.ts",
                "",
                "import { B } from \"@/ui/Button\";\n"
            ),
            [finding(1, "imports @/ui/Button, forbidden by \"no-ui\"")]
        );
    }

    #[test]
    fn invalid_pattern() {
        let err = ArchitectureAnalyzer::new(&[rule("domain-is-pure", &["src/[domain"], &["x"])])
            .expect_err("invalid pattern");
        assert_eq!(err.rule, "domain-is-pure");
        assert_eq!(err.pattern, "src/[domain");
        assert!(
            err.to_string()
                .starts_with("rule \"domain-is-pure\": invalid pattern \"src/[domain\": "),
            "{err}"
        );
    }

    // Requirement: Only new violations are reported

    #[test]
    fn existing_violation() {
        assert_eq!(
            fixture!(&[domain_is_pure()], None, "src/domain/order.ts", "existing"),
            []
        );
    }

    #[test]
    fn moved_within_the_domain() {
        assert_eq!(
            fixture!(
                &[domain_is_pure()],
                Some("src/domain/order.ts"),
                "src/domain/sales/order.ts",
                "moved_within"
            ),
            []
        );
    }

    #[test]
    fn moved_into_the_domain() {
        assert_eq!(
            fixture!(
                &[domain_is_pure()],
                Some("src/app/order.ts"),
                "src/domain/order.ts",
                "moved_into"
            ),
            [finding(
                1,
                "imports src/ui/Button (\"../ui/Button\"), forbidden by \"domain-is-pure\""
            )]
        );
    }

    #[test]
    fn rule_added_in_the_same_change() {
        // The analyzer applies the head's rules to both versions.
        assert_eq!(
            fixture!(
                &[domain_is_pure()],
                None,
                "src/domain/order.ts",
                "rule_added"
            ),
            [finding(
                2,
                "imports src/ui/B (\"../ui/B\"), forbidden by \"domain-is-pure\""
            )]
        );
    }

    #[test]
    fn added_file() {
        assert_eq!(
            run(
                &[domain_is_pure()],
                None,
                "src/domain/new.ts",
                "",
                "import \"../ui/x\";\n"
            ),
            [finding(
                1,
                "imports src/ui/x (\"../ui/x\"), forbidden by \"domain-is-pure\""
            )]
        );
    }

    // Requirement: Forbidden import finding

    #[test]
    fn package_import_message() {
        let rules = [rule("no-db-in-ui", &["src/ui/**"], &["@prisma/client"])];
        assert_eq!(
            fixture!(&rules, None, "src/ui/List.tsx", "package"),
            [finding(
                2,
                "imports @prisma/client, forbidden by \"no-db-in-ui\""
            )]
        );
    }

    #[test]
    fn same_target_imported_twice() {
        assert_eq!(
            fixture!(&[domain_is_pure()], None, "src/domain/a.ts", "twice"),
            [finding(
                2,
                "imports src/ui/Button (\"../ui/Button\"), forbidden by \"domain-is-pure\""
            )]
        );
    }

    #[test]
    fn two_rules_forbid_the_same_import() {
        let rules = [
            domain_is_pure(),
            rule("no-buttons", &["src/**"], &["src/ui/Button"]),
        ];
        let messages: Vec<String> = fixture!(&rules, None, "src/domain/order.ts", "new_import")
            .into_iter()
            .map(|(_, m)| m)
            .collect();
        assert_eq!(messages.len(), 2, "{messages:?}");
        assert!(messages[0].ends_with("\"domain-is-pure\""));
        assert!(messages[1].ends_with("\"no-buttons\""));
    }

    #[test]
    fn no_rules_no_findings() {
        let analyzer = ArchitectureAnalyzer::new(&[]).expect("no rules");
        assert!(analyzer.is_empty());
    }
}
