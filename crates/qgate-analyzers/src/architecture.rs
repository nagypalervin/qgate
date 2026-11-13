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
//! Patterns are [`PathPatterns`]: globs over
//! `/`-separated paths, where a pattern without glob characters also
//! matches everything below it (`react` matches `react/jsx-runtime`), and
//! `dir/**` also matches `dir`.
//!
//! C# files have no imports but `using` directives: a rule's
//! `to_namespaces` ([`NamespacePatterns`]) matches the names they use,
//! and `to` never does (design D5 of `add-csharp-architecture`).

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use qgate_core::{Finding, Location, Severity};
use qgate_lang::FileModel;

use crate::patterns::{InvalidPattern, NamespacePatterns, PathPatterns};
// The spelling tests below call it directly.
#[cfg(test)]
use crate::patterns::normalize;
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
    /// Patterns for the namespaces C# `using` directives name.
    pub to_namespaces: Vec<String>,
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
    from: PathPatterns,
    to: PathPatterns,
    to_namespaces: NamespacePatterns,
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
                let named = |e: InvalidPattern| PatternError {
                    rule: rule.name.clone(),
                    pattern: e.pattern,
                    detail: e.detail,
                };
                Ok(CompiledRule {
                    name: rule.name.clone(),
                    from: compile(&rule.name, &rule.from)?,
                    to: compile(&rule.name, &rule.to)?,
                    to_namespaces: NamespacePatterns::new(&rule.to_namespaces).map_err(named)?,
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
    /// target), the line of the first import or using of the target and
    /// how the finding shows it. A file has imports (TypeScript) or usings
    /// (C#), never both, so the two kinds of target cannot meet in a key.
    fn violations(
        &self,
        path: &str,
        model: &FileModel,
    ) -> BTreeMap<(usize, String), (u32, String)> {
        let mut found = BTreeMap::new();
        for (index, rule) in self.rules.iter().enumerate() {
            if !rule.from.matches(path) {
                continue;
            }
            for import in &model.imports {
                let Some(target) = target(path, &import.specifier) else {
                    continue;
                };
                if rule.to.matches(&target) {
                    let shown = if import.specifier == target {
                        format!("imports {target}")
                    } else {
                        format!("imports {target} (\"{}\")", import.specifier)
                    };
                    found.entry((index, target)).or_insert((import.line, shown));
                }
            }
            for using in &model.usings {
                if rule.to_namespaces.matches(&using.name) {
                    found
                        .entry((index, using.name.clone()))
                        .or_insert_with(|| (using.line, format!("uses {}", using.name)));
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
            .map(|((index, _), (line, shown))| Finding {
                rule: RULE.to_owned(),
                severity: Severity::Error,
                message: format!("{shown}, forbidden by \"{}\"", self.rules[index].name),
                location: Location {
                    path: change.path.clone(),
                    line: Some(line),
                    end_line: None,
                },
                metric: None,
                related: Vec::new(),
            })
            .collect()
    }
}

/// A path as git wrote it: revision mode builds every path from git's
/// `/`-separated output.
fn slash_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The target of `specifier` imported by the file at `importer` (design
/// D7): a relative specifier resolved from the importer's directory, or
/// the specifier itself. `None` if a relative specifier leaves the root.
fn target(importer: &str, specifier: &str) -> Option<String> {
    if specifier.is_empty() {
        return None;
    }
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

/// Compile the patterns of one side of a rule (design D3), naming the
/// rule in an error.
fn compile(rule: &str, patterns: &[String]) -> Result<PathPatterns, PatternError> {
    PathPatterns::new(patterns).map_err(|e| PatternError {
        rule: rule.to_owned(),
        pattern: e.pattern,
        detail: e.detail,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use qgate_lang::{Language, parse};

    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|&s| s.to_owned()).collect()
    }

    fn rule(name: &str, from: &[&str], to: &[&str]) -> ForbidRule {
        ForbidRule {
            name: name.to_owned(),
            from: strings(from),
            to: strings(to),
            to_namespaces: Vec::new(),
        }
    }

    fn namespace_rule(name: &str, from: &[&str], to_namespaces: &[&str]) -> ForbidRule {
        ForbidRule {
            name: name.to_owned(),
            from: strings(from),
            to: Vec::new(),
            to_namespaces: strings(to_namespaces),
        }
    }

    fn domain_uses_no_infrastructure() -> ForbidRule {
        namespace_rule(
            "domain-is-pure",
            &["src/Shop.Domain/**"],
            &["Shop.Infrastructure", "Microsoft.EntityFrameworkCore"],
        )
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
        run_in(Language::TypeScript, rules, old_path, path, before, after)
    }

    /// [`run`] for sources in `language`.
    fn run_in(
        language: Language,
        rules: &[ForbidRule],
        old_path: Option<&str>,
        path: &str,
        before: &str,
        after: &str,
    ) -> Vec<(u32, String)> {
        let change = FileChange {
            path: PathBuf::from(path),
            old_path: old_path.map(PathBuf::from),
            before: parse(language, before).expect("before parses"),
            after: parse(language, after).expect("after parses"),
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

    macro_rules! cs_fixture {
        ($rules:expr, $old:expr, $path:expr, $case:literal) => {
            run_in(
                Language::CSharp,
                $rules,
                $old,
                $path,
                include_str!(concat!("../fixtures/architecture/", $case, "/before.cs")),
                include_str!(concat!("../fixtures/architecture/", $case, "/after.cs")),
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
            .matches(path)
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
    fn equivalent_spellings_are_normalized() {
        assert!(matches("src/domain/", "src/domain/a.ts"));
        assert!(matches("./src/ui/**", "src/ui/Button"));
        assert!(matches("././react/", "react/jsx-runtime"));
        assert!(matches("src//domain/./**", "src/domain/a.ts"));
        assert_eq!(normalize("./src//ui/./x/").as_deref(), Ok("src/ui/x"));
    }

    #[test]
    fn equivalent_spellings_through_the_analyzer() {
        let rules = [rule("domain-is-pure", &["./src//domain/"], &["src/ui/**"])];
        assert_eq!(
            fixture!(&rules, None, "src/domain/order.ts", "new_import"),
            [finding(
                3,
                "imports src/ui/Button (\"../ui/Button\"), forbidden by \"domain-is-pure\""
            )]
        );
    }

    #[test]
    fn braces_and_escapes_keep_their_glob_meaning() {
        assert!(matches("src/{ui,infra}/**", "src/infra"));
        assert!(matches("src/{ui,infra}/**", "src/ui/Button"));
        assert!(!matches("src/{ui,infra}/**", "src/app/x"));
        assert!(matches("src/\\*", "src/*"));
        assert!(!matches("src/\\*", "src/x"));
        // Not rewritten inside braces.
        assert_eq!(normalize("src/{a//b,c}").as_deref(), Ok("src/{a//b,c}"));
    }

    #[test]
    fn absolute_and_empty_patterns_are_invalid() {
        for (pattern, detail) in [
            ("/src/**", "remove the leading `/`"),
            (".//src/domain/**", "remove the leading `/`"),
            ("././/src/domain", "remove the leading `/`"),
            ("src/../ui/**", "`..` cannot match"),
            ("../{src/ui,x}/**", "`..` cannot match"),
            ("src/{ui,x}/../ui/**", "`..` cannot match"),
            ("src/{..,ui}/**", "`..` cannot match"),
            ("./", "the pattern is empty"),
            (".", "the pattern is empty"),
            ("", "the pattern is empty"),
        ] {
            let err = ArchitectureAnalyzer::new(&[rule("r", &[pattern], &["x"])])
                .expect_err("invalid pattern");
            assert!(err.detail.contains(detail), "{pattern:?}: {err}");
        }
    }

    #[test]
    fn index_import_of_a_directory() {
        assert_eq!(
            run(
                &[domain_is_pure()],
                None,
                "src/domain/a.ts",
                "",
                "import { ui } from \"../ui\";\n"
            ),
            [finding(
                1,
                "imports src/ui (\"../ui\"), forbidden by \"domain-is-pure\""
            )]
        );
    }

    #[test]
    fn empty_specifier_has_no_target() {
        assert_eq!(target("src/a.ts", ""), None);
        let rules = [rule("r", &["**"], &["**"])];
        assert_eq!(run(&rules, None, "src/a.ts", "", "import \"\";\n"), []);
    }

    #[test]
    fn suffixes_and_prefixes_stay_part_of_the_target() {
        let raw = target("src/domain/a.ts", "../ui/B?raw").expect("a target");
        assert_eq!(raw, "src/ui/B?raw");
        assert!(matches("src/ui/**", &raw));
        assert!(!matches("src/ui/B", &raw));
        assert!(!matches("fs", "node:fs"));
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

    // C# usings (add-csharp-architecture)

    // "Domain using the infrastructure"
    #[test]
    fn domain_using_the_infrastructure() {
        assert_eq!(
            cs_fixture!(
                &[domain_uses_no_infrastructure()],
                None,
                "src/Shop.Domain/Order.cs",
                "csharp_new_using"
            ),
            [finding(
                2,
                "uses Shop.Infrastructure.Data, forbidden by \"domain-is-pure\""
            )]
        );
        // Outside `from`.
        assert_eq!(
            cs_fixture!(
                &[domain_uses_no_infrastructure()],
                None,
                "src/Shop.Web/Order.cs",
                "csharp_new_using"
            ),
            []
        );
    }

    // "Existing using, written another way"
    #[test]
    fn existing_using_written_another_way() {
        assert_eq!(
            cs_fixture!(
                &[domain_uses_no_infrastructure()],
                None,
                "src/Shop.Domain/Order.cs",
                "csharp_existing"
            ),
            []
        );
    }

    // "Moved within the domain" and "Moved into the domain", for C#.
    #[test]
    fn renamed_csharp_files() {
        let rules = [domain_uses_no_infrastructure()];
        assert_eq!(
            cs_fixture!(
                &rules,
                Some("src/Shop.Domain/Order.cs"),
                "src/Shop.Domain/Sales/Order.cs",
                "csharp_existing"
            ),
            []
        );
        assert_eq!(
            cs_fixture!(
                &rules,
                Some("src/Shop.Application/Order.cs"),
                "src/Shop.Domain/Order.cs",
                "csharp_existing"
            ),
            [finding(
                1,
                "uses Shop.Infrastructure.Data, forbidden by \"domain-is-pure\""
            )]
        );
    }

    // "Same namespace used twice"
    #[test]
    fn same_namespace_used_twice() {
        assert_eq!(
            cs_fixture!(
                &[domain_uses_no_infrastructure()],
                None,
                "src/Shop.Domain/Order.cs",
                "csharp_twice"
            ),
            [finding(
                1,
                "uses Microsoft.EntityFrameworkCore, forbidden by \"domain-is-pure\""
            )]
        );
    }

    // "Each list applies to its language"
    #[test]
    fn each_list_applies_to_its_language() {
        // A path pattern that would match any C# name, and a namespace
        // pattern that would match any TypeScript target.
        let rules = [
            rule("imports", &["src/**"], &["**"]),
            namespace_rule("usings", &["src/**"], &["**"]),
        ];
        let messages: Vec<String> = fixture!(&rules, None, "src/domain/order.ts", "new_import")
            .into_iter()
            .map(|(_, m)| m)
            .collect();
        assert_eq!(
            messages,
            ["imports src/ui/Button (\"../ui/Button\"), forbidden by \"imports\""]
        );
        let messages: Vec<String> = cs_fixture!(&rules, None, "src/Order.cs", "csharp_new_using")
            .into_iter()
            .map(|(_, m)| m)
            .collect();
        assert_eq!(
            messages,
            ["uses Shop.Infrastructure.Data, forbidden by \"usings\""]
        );
    }

    // "Empty segment": the rule is named, as for `from` and `to`.
    #[test]
    fn invalid_namespace_pattern() {
        let error = ArchitectureAnalyzer::new(&[namespace_rule(
            "domain-is-pure",
            &["src/**"],
            &["Shop..Infrastructure"],
        )])
        .expect_err("invalid pattern");
        assert_eq!(error.rule, "domain-is-pure");
        assert_eq!(error.pattern, "Shop..Infrastructure");
        assert!(
            error
                .to_string()
                .starts_with("rule \"domain-is-pure\": invalid pattern \"Shop..Infrastructure\": "),
            "{error}"
        );
    }

    #[test]
    fn no_rules_no_findings() {
        let analyzer = ArchitectureAnalyzer::new(&[]).expect("no rules");
        assert!(analyzer.is_empty());
    }
}
