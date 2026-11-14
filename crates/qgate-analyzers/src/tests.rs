//! Code that a change edits without changing its tests (M5a).
//!
//! A repository-level analysis, not an [`Analyzer`](crate::Analyzer), as
//! duplication is: whether a file has a test, and whether that test
//! changed, depends on other files of the change and on the head tree
//! (design D2 of `add-test-gap-analysis`). It works on paths and models
//! only; the caller reads git.
//!
//! - **Tests by convention** (design D3): a TypeScript source
//!   `cart.service.ts` is tested by `cart.service.spec.ts` or
//!   `cart.service.test.ts` beside it, and every TypeScript file below a
//!   `spec`, `test`, `tests`, `__tests__` or `e2e` directory is test
//!   code; a C# source `Order.cs` of the project `Shop.Domain` is tested
//!   by an `OrderTests.cs` or `OrderTest.cs` in a test project
//!   `Shop.Domain.<X>Tests` (or `<X>Test`), the project part compared
//!   without regard to ASCII case.
//! - **Feature test projects** (acceptance, integration, functional,
//!   end-to-end) hold tests, but a missing test is never expected there.
//! - **Projects that test other code** (design D5): a new file without a
//!   test is reported only when its TypeScript package or its C# project
//!   tests something, so a repository without tests stays quiet.
//! - **Changed units** (design D4): units of the head version that are
//!   new or whose fingerprint differs from the base version's unit of the
//!   same name. A file with none needs no test change.
//!
//! Both findings are Warnings, one per source file: a refactor can rightly
//! leave its tests alone.

use std::collections::{HashMap, HashSet};

use qgate_core::{Finding, Location, Severity};
use qgate_lang::Function;

use crate::FileChange;
use crate::patterns::PathPatterns;

/// Rule id of a source file whose units changed while none of its tests
/// did.
pub const UNCHANGED_TEST_RULE: &str = "tests/unchanged-test";

/// Rule id of a new source file without a test, in a project that tests
/// other code.
pub const MISSING_TEST_RULE: &str = "tests/missing-test";

/// The findings of a revision check's changes.
///
/// - `changes`: the analyzed changed files (supported, not excluded, not
///   generated, parsed);
/// - `head_paths`: every path of the head revision;
/// - `changed_paths`: the head paths of every file the change added,
///   edited or renamed, before any file selection, so a test counts as
///   changed even when `[files] exclude` names it;
/// - `exclude`: the `[tests] exclude` patterns, source paths that need no
///   test.
///
/// Findings come in the order of `changes`.
pub fn analyze<'a>(
    changes: &[FileChange],
    head_paths: impl IntoIterator<Item = &'a str>,
    changed_paths: impl IntoIterator<Item = &'a str>,
    exclude: &PathPatterns,
) -> Vec<Finding> {
    let conventions = Conventions::new(head_paths);
    let edited: HashSet<&str> = changed_paths.into_iter().collect();
    changes
        .iter()
        .filter_map(|change| finding(&conventions, &edited, exclude, change))
        .collect()
}

/// The finding for one changed file, if it needs one.
fn finding(
    conventions: &Conventions<'_>,
    changed: &HashSet<&str>,
    exclude: &PathPatterns,
    change: &FileChange,
) -> Option<Finding> {
    let path = change.path.to_str()?;
    if exclude.matches(path) {
        return None;
    }
    let Role::Source { tests, expected } = conventions.role(path) else {
        return None;
    };
    let units = changed_units(change);
    let first = units.first()?;
    let names = match units.len() {
        1 => first.name.clone(),
        n => format!("{} and {} more", first.name, n - 1),
    };
    let (rule, message) = if let Some(test) = tests.first() {
        if tests.iter().any(|t| changed.contains(t)) {
            return None;
        }
        let more = match tests.len() {
            1 => String::new(),
            n => format!(", and {} more", n - 1),
        };
        (
            UNCHANGED_TEST_RULE,
            format!("{names} changed, but not its test {test}{more}"),
        )
    } else {
        // Only new code: a file that was untested before is not reported.
        let expected = expected.filter(|_| change.before.functions.is_empty())?;
        let verb = if units.len() == 1 { "has" } else { "have" };
        (
            MISSING_TEST_RULE,
            format!("{names} {verb} no test: expected {expected}"),
        )
    };
    Some(Finding {
        rule: rule.to_owned(),
        severity: Severity::Warning,
        message,
        location: Location {
            path: change.path.clone(),
            line: Some(first.line),
            end_line: None,
        },
        metric: None,
        related: Vec::new(),
        hotspot: None,
    })
}

/// The units of the head version that are new, or whose code differs
/// from the base version's unit of the same name, in source order.
fn changed_units(change: &FileChange) -> Vec<&Function> {
    let before: HashMap<&str, u64> = change
        .before
        .functions
        .iter()
        .map(|f| (f.name.as_str(), f.fingerprint))
        .collect();
    change
        .after
        .functions
        .iter()
        .filter(|f| before.get(f.name.as_str()) != Some(&f.fingerprint))
        .collect()
}

/// The TypeScript extensions a source or a test can have.
const TS_EXTENSIONS: [&str; 4] = ["ts", "mts", "cts", "tsx"];

/// Directories whose TypeScript files are test code, helpers included.
const TEST_DIRS: [&str; 5] = ["spec", "test", "tests", "__tests__", "e2e"];

/// The last name segments of feature test projects, lowercase, without
/// the final `test` or `tests`.
const FEATURE_TESTS: [&str; 5] = ["acceptance", "integration", "functional", "endtoend", "e2e"];

/// What the head revision's paths say about tests: which files are tests,
/// which tests a source has, and whether its project tests other code.
/// Built once per run from every path of the head revision.
#[derive(Debug)]
pub(crate) struct Conventions<'a> {
    /// Every path of the head revision.
    head: HashSet<&'a str>,
    /// Directories that hold a `package.json`.
    packages: HashSet<&'a str>,
    /// Directories with a TypeScript test at any depth below them; `""`
    /// is the repository root.
    tested_dirs: HashSet<&'a str>,
    /// The project of each directory that holds a `.csproj`, by name.
    projects: HashMap<&'a str, &'a str>,
    /// The test projects of each project, by the project's name in
    /// lowercase (names ignore ASCII case), sorted.
    test_projects: HashMap<String, Vec<&'a str>>,
    /// The paths of the files in each test project, by project and file
    /// name.
    test_files: HashMap<(&'a str, &'a str), Vec<&'a str>>,
}

/// What a changed file is to the analysis.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Role<'a> {
    /// A test: never a source, so never reported.
    Test,
    /// A source with its tests, sorted by path, and the test the
    /// `tests/missing-test` message expects. `expected` is `None` when
    /// the file's project tests nothing, so a missing test is not
    /// reported.
    Source {
        /// The tests that exist in the head revision.
        tests: Vec<&'a str>,
        /// The expected test, as the message names it.
        expected: Option<String>,
    },
    /// Neither: a C# file outside any project, or a file in a language
    /// without a convention.
    Other,
}

impl<'a> Conventions<'a> {
    /// Index the paths of the head revision.
    pub(crate) fn new(head_paths: impl IntoIterator<Item = &'a str>) -> Self {
        let head: HashSet<&str> = head_paths.into_iter().collect();
        let mut packages = HashSet::new();
        let mut tested_dirs = HashSet::new();
        let mut projects: HashMap<&str, &str> = HashMap::new();
        for &path in &head {
            let (dir, name) = split_path(path);
            if name == "package.json" {
                packages.insert(dir);
            } else if is_ts_test(dir, name) {
                tested_dirs.extend(ancestors(dir));
            } else if let Some(project) = name.strip_suffix(".csproj") {
                // Two project files in one directory: the first by name.
                projects
                    .entry(dir)
                    .and_modify(|p| *p = (*p).min(project))
                    .or_insert(project);
            }
        }
        let mut test_projects: HashMap<String, Vec<&str>> = HashMap::new();
        for &name in projects.values() {
            if let Some((parent, _)) = name.rsplit_once('.').filter(|_| is_test_project(name)) {
                test_projects
                    .entry(parent.to_ascii_lowercase())
                    .or_default()
                    .push(name);
            }
        }
        for names in test_projects.values_mut() {
            names.sort_unstable();
            names.dedup();
        }
        let mut conventions = Self {
            head,
            packages,
            tested_dirs,
            projects,
            test_projects,
            test_files: HashMap::new(),
        };
        let mut test_files: HashMap<(&str, &str), Vec<&str>> = HashMap::new();
        for &path in &conventions.head {
            let (dir, name) = split_path(path);
            if extension(name) != Some("cs") {
                continue;
            }
            if let Some(project) = conventions.project_of(dir).filter(|p| is_test_project(p)) {
                test_files.entry((project, name)).or_default().push(path);
            }
        }
        for paths in test_files.values_mut() {
            paths.sort_unstable();
        }
        conventions.test_files = test_files;
        conventions
    }

    /// What the file at `path` is: a test, a source with its tests, or
    /// neither.
    pub(crate) fn role(&self, path: &str) -> Role<'a> {
        let (dir, name) = split_path(path);
        if let Some((stem, ext)) = ts_name(name) {
            if is_ts_test(dir, name) {
                return Role::Test;
            }
            return self.typescript_source(dir, stem, ext);
        }
        if let Some(stem) = name.strip_suffix(".cs") {
            let Some(project) = self.project_of(dir) else {
                return Role::Other;
            };
            if is_test_project(project) {
                return Role::Test;
            }
            return self.csharp_source(project, stem);
        }
        Role::Other
    }

    fn typescript_source(&self, dir: &str, stem: &str, ext: &str) -> Role<'a> {
        let in_dir = |name: String| {
            if dir.is_empty() {
                name
            } else {
                format!("{dir}/{name}")
            }
        };
        let mut tests: Vec<&'a str> = ["spec", "test"]
            .iter()
            .flat_map(|kind| {
                TS_EXTENSIONS
                    .iter()
                    .map(move |e| in_dir(format!("{stem}.{kind}.{e}")))
            })
            .filter_map(|candidate| self.head.get(candidate.as_str()).copied())
            .collect();
        tests.sort_unstable();
        // The package is the nearest directory with a `package.json`, or
        // the repository root.
        let package = ancestors(dir)
            .find(|d| self.packages.contains(d))
            .unwrap_or("");
        let expected = self
            .tested_dirs
            .contains(package)
            .then(|| in_dir(format!("{stem}.spec.{ext}")));
        Role::Source { tests, expected }
    }

    fn csharp_source(&self, project: &str, stem: &str) -> Role<'a> {
        let test_projects = self
            .test_projects
            .get(&project.to_ascii_lowercase())
            .map_or(&[][..], Vec::as_slice);
        let names = [format!("{stem}Tests.cs"), format!("{stem}Test.cs")];
        let mut tests: Vec<&'a str> = test_projects
            .iter()
            .flat_map(|&test_project| {
                names
                    .iter()
                    .filter_map(move |name| self.test_files.get(&(test_project, name.as_str())))
            })
            .flatten()
            .copied()
            .collect();
        tests.sort_unstable();
        tests.dedup();
        // Feature test projects hold tests, but a missing one is expected
        // only in a class test project.
        let expected = test_projects
            .iter()
            .find(|name| !is_feature_test_project(name))
            .map(|first| format!("{stem}Tests.cs in {first}"));
        Role::Source { tests, expected }
    }

    /// The project of a file in `dir`: the one whose `.csproj` is in
    /// `dir` or the nearest directory above it.
    fn project_of(&self, dir: &str) -> Option<&'a str> {
        ancestors(dir).find_map(|d| self.projects.get(d).copied())
    }
}

/// The directory and the file name of a repository path; the directory is
/// `""` at the root.
fn split_path(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

/// `dir` and every directory above it, ending with the root, `""`.
fn ancestors(dir: &str) -> impl Iterator<Item = &str> {
    let mut next = Some(dir);
    std::iter::from_fn(move || {
        let current = next?;
        next = if current.is_empty() {
            None
        } else {
            Some(current.rsplit_once('/').map_or("", |(parent, _)| parent))
        };
        Some(current)
    })
}

/// The stem and extension of a TypeScript file name, if it is one.
fn ts_name(name: &str) -> Option<(&str, &str)> {
    name.rsplit_once('.')
        .filter(|(_, ext)| TS_EXTENSIONS.contains(ext))
}

/// Whether a TypeScript file in `dir` is a test: `.spec` or `.test`
/// before its extension, or a test directory in its path.
fn is_ts_test(dir: &str, name: &str) -> bool {
    ts_name(name).is_some_and(|(stem, _)| {
        matches!(extension(stem), Some("spec" | "test"))
            || dir.split('/').any(|segment| TEST_DIRS.contains(&segment))
    })
}

/// The part of a file name after its last `.`, compared as written:
/// paths are matched byte for byte, as git stores them (design D3).
fn extension(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(_, extension)| extension)
}

/// Whether a project is a test project: the last segment of its name
/// ends in `Test` or `Tests`.
fn is_test_project(name: &str) -> bool {
    let last = name.rsplit('.').next().unwrap_or(name);
    last.ends_with("Test") || last.ends_with("Tests")
}

/// Whether a test project tests features rather than classes: its last
/// name segment, ignoring ASCII case, is one of [`FEATURE_TESTS`]
/// followed by `test` or `tests`.
fn is_feature_test_project(name: &str) -> bool {
    let last = name.rsplit('.').next().unwrap_or(name).to_ascii_lowercase();
    let last = last.strip_suffix('s').unwrap_or(&last);
    last.strip_suffix("test")
        .is_some_and(|kind| FEATURE_TESTS.contains(&kind))
}

// The analyzer is called `tests`, and test modules here are called
// `tests` too (CLAUDE.md), so the two names meet.
#[allow(clippy::module_inception)]
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use qgate_lang::{Language, parse};

    use super::*;

    fn conventions<'a>(paths: &[&'a str]) -> Conventions<'a> {
        Conventions::new(paths.iter().copied())
    }

    fn source<'a>(tests: &[&'a str], expected: Option<&str>) -> Role<'a> {
        Role::Source {
            tests: tests.to_vec(),
            expected: expected.map(str::to_owned),
        }
    }

    // test-analysis: "TypeScript tests"
    #[test]
    fn angular_component_and_its_spec() {
        let head = conventions(&[
            "package.json",
            "src/app/cart/cart.component.ts",
            "src/app/cart/cart.component.spec.ts",
        ]);
        assert_eq!(
            head.role("src/app/cart/cart.component.ts"),
            source(
                &["src/app/cart/cart.component.spec.ts"],
                Some("src/app/cart/cart.component.spec.ts")
            )
        );
        assert_eq!(head.role("src/app/cart/cart.component.spec.ts"), Role::Test);
    }

    #[test]
    fn every_typescript_extension_and_both_suffixes() {
        let head = conventions(&[
            "src/order.mts",
            "src/order.spec.ts",
            "src/order.test.tsx",
            "src/order.spec.mts",
            "src/view.tsx",
        ]);
        assert_eq!(
            head.role("src/order.mts"),
            source(
                &[
                    "src/order.spec.mts",
                    "src/order.spec.ts",
                    "src/order.test.tsx"
                ],
                Some("src/order.spec.mts")
            )
        );
        for test in ["src/order.spec.mts", "src/x.test.cts", "src/y.spec.tsx"] {
            assert_eq!(head.role(test), Role::Test, "{test}");
        }
        // Expected with the source's own extension.
        assert_eq!(
            head.role("src/view.tsx"),
            source(&[], Some("src/view.spec.tsx"))
        );
    }

    #[test]
    fn a_test_elsewhere_is_no_test_of_the_file() {
        let head = conventions(&["src/app/cart.service.ts", "test/cart.service.spec.ts"]);
        assert_eq!(
            head.role("src/app/cart.service.ts"),
            source(&[], Some("src/app/cart.service.spec.ts"))
        );
    }

    #[test]
    fn a_file_at_the_root() {
        let head = conventions(&["index.ts", "index.spec.ts"]);
        assert_eq!(
            head.role("index.ts"),
            source(&["index.spec.ts"], Some("index.spec.ts"))
        );
    }

    // test-analysis: "Projects that test other code"
    #[test]
    fn a_repository_without_specs_expects_none() {
        let head = conventions(&["package.json", "src/format.ts"]);
        assert_eq!(head.role("src/format.ts"), source(&[], None));
    }

    #[test]
    fn a_monorepo_package_tests_only_itself() {
        let head = conventions(&[
            "apps/web/package.json",
            "apps/web/src/a.spec.ts",
            "libs/util/package.json",
            "libs/util/src/b.ts",
            "tools/c.ts",
        ]);
        assert_eq!(
            head.role("apps/web/src/new.ts"),
            source(&[], Some("apps/web/src/new.spec.ts"))
        );
        assert_eq!(head.role("libs/util/src/new.ts"), source(&[], None));
        // No package above: the root, which has a spec below it.
        assert_eq!(
            head.role("tools/new.ts"),
            source(&[], Some("tools/new.spec.ts"))
        );
    }

    // test-analysis: "C# projects"
    #[test]
    fn test_project_names() {
        for name in [
            "Core.Test",
            "Shop.Domain.UnitTests",
            "Application.FunctionalTests",
            "Tests",
        ] {
            assert!(is_test_project(name), "{name}");
        }
        for name in ["Shop.Testing", "Shop", "Tests.Shop", "Shop.Testable"] {
            assert!(!is_test_project(name), "{name}");
        }
    }

    const CLEAN: &[&str] = &[
        "src/Domain/Domain.csproj",
        "src/Domain/ValueObjects/Colour.cs",
        "src/Domain/Entities/TodoItem.cs",
        "src/Application/Application.csproj",
        "src/Application/TodoItems.cs",
        "src/Web/Web.csproj",
        "src/Web/Program.cs",
        "tests/Domain.UnitTests/Domain.UnitTests.csproj",
        "tests/Domain.UnitTests/ValueObjects/ColourTests.cs",
        "tests/Application.UnitTests/Application.UnitTests.csproj",
        "tests/Application.UnitTests/TodoItemsTests.cs",
        "tests/Application.FunctionalTests/Application.FunctionalTests.csproj",
        "tests/Application.FunctionalTests/Commands/TodoItemsTests.cs",
        "scripts/Build.cs",
    ];

    // test-analysis: "C# tests"
    #[test]
    fn clean_architecture_layout() {
        let head = conventions(CLEAN);
        assert_eq!(
            head.role("src/Domain/ValueObjects/Colour.cs"),
            source(
                &["tests/Domain.UnitTests/ValueObjects/ColourTests.cs"],
                Some("ColourTests.cs in Domain.UnitTests")
            )
        );
        assert_eq!(
            head.role("src/Domain/Entities/TodoItem.cs"),
            source(&[], Some("TodoItemTests.cs in Domain.UnitTests"))
        );
        assert_eq!(
            head.role("tests/Domain.UnitTests/ValueObjects/ColourTests.cs"),
            Role::Test
        );
    }

    #[test]
    fn several_test_projects() {
        let head = conventions(CLEAN);
        assert_eq!(
            head.role("src/Application/TodoItems.cs"),
            source(
                &[
                    "tests/Application.FunctionalTests/Commands/TodoItemsTests.cs",
                    "tests/Application.UnitTests/TodoItemsTests.cs",
                ],
                // The functional tests count, but a missing test is
                // expected in the class test project.
                Some("TodoItemsTests.cs in Application.UnitTests")
            )
        );
    }

    #[test]
    fn a_project_without_a_test_project_and_a_file_outside_any() {
        let head = conventions(CLEAN);
        assert_eq!(head.role("src/Web/Program.cs"), source(&[], None));
        assert_eq!(head.role("scripts/Build.cs"), Role::Other);
    }

    #[test]
    fn the_test_suffix_without_s() {
        let head = conventions(&[
            "src/Core/Core.csproj",
            "src/Core/Services/UserService.cs",
            "test/Core.Test/Core.Test.csproj",
            "test/Core.Test/Services/UserServiceTest.cs",
        ]);
        assert_eq!(
            head.role("src/Core/Services/UserService.cs"),
            source(
                &["test/Core.Test/Services/UserServiceTest.cs"],
                Some("UserServiceTests.cs in Core.Test")
            )
        );
    }

    #[test]
    fn a_test_project_of_another_project() {
        let head = conventions(&[
            "src/Shop/Shop.csproj",
            "src/Shop/Order.cs",
            "src/Shop.Api/Shop.Api.csproj",
            "tests/Shop.Api.Tests/Shop.Api.Tests.csproj",
            "tests/Shop.Api.Tests/OrderTests.cs",
        ]);
        assert_eq!(head.role("src/Shop/Order.cs"), source(&[], None));
    }

    #[test]
    fn nested_projects_and_two_project_files_in_one_directory() {
        let head = conventions(&[
            "src/Shop/Shop.csproj",
            "src/Shop/Legacy/Legacy.csproj",
            "src/Shop/Legacy/Old.cs",
            "src/Two/B.csproj",
            "src/Two/A.csproj",
            "src/Two/Thing.cs",
            "tests/A.Tests/A.Tests.csproj",
            "tests/A.Tests/ThingTests.cs",
        ]);
        // The nearest project file wins.
        assert_eq!(head.role("src/Shop/Legacy/Old.cs"), source(&[], None));
        // The first project file by name.
        assert_eq!(
            head.role("src/Two/Thing.cs"),
            source(
                &["tests/A.Tests/ThingTests.cs"],
                Some("ThingTests.cs in A.Tests")
            )
        );
    }

    // "TypeScript test code" (decided after the evaluation, design D3)
    #[test]
    fn test_code_by_directory() {
        let head = conventions(&["package.json", "src/a.spec.ts"]);
        for path in [
            "src/app/spec/test-utils.ts",
            "e2e/cart-helpers.ts",
            "test/setup.ts",
            "packages/x/tests/fixtures/data.ts",
            "src/__tests__/helper.tsx",
        ] {
            assert_eq!(head.role(path), Role::Test, "{path}");
        }
        for path in ["src/testing/format.ts", "src/specs/x.ts", "e2e.ts"] {
            assert!(matches!(head.role(path), Role::Source { .. }), "{path}");
        }
        // A test directory alone makes its package one that tests code.
        let head = conventions(&["package.json", "e2e/cart.ts", "src/b.ts"]);
        assert_eq!(head.role("src/b.ts"), source(&[], Some("src/b.spec.ts")));
    }

    // "Names that differ in case"
    #[test]
    fn project_names_ignore_case() {
        let head = conventions(&[
            "src/Sso/Sso.csproj",
            "src/Sso/AccountController.cs",
            "test/SSO.Test/SSO.Test.csproj",
            "test/SSO.Test/AccountControllerTest.cs",
            "test/SSO.Test/accountcontrollertests.cs",
        ]);
        assert_eq!(
            head.role("src/Sso/AccountController.cs"),
            source(
                &["test/SSO.Test/AccountControllerTest.cs"],
                Some("AccountControllerTests.cs in SSO.Test")
            )
        );
    }

    // "Feature test projects"
    #[test]
    fn feature_test_projects() {
        for name in [
            "Web.AcceptanceTests",
            "Sso.IntegrationTest",
            "Api.IntegrationTests",
            "Application.FunctionalTests",
            "Shop.EndToEndTests",
            "Shop.E2ETests",
            "Shop.e2etest",
        ] {
            assert!(is_feature_test_project(name), "{name}");
        }
        for name in [
            "Shop.UnitTests",
            "Shop.Tests",
            "Core.Test",
            "Shop.ArchitectureTests",
        ] {
            assert!(!is_feature_test_project(name), "{name}");
        }
        let head = conventions(&[
            "src/Web/Web.csproj",
            "src/Web/Handler.cs",
            "tests/Web.AcceptanceTests/Web.AcceptanceTests.csproj",
            "tests/Web.AcceptanceTests/HandlerTests.cs",
            "src/Application/Application.csproj",
            "src/Application/Orders.cs",
            "tests/Application.FunctionalTests/Application.FunctionalTests.csproj",
            "tests/Application.UnitTests/Application.UnitTests.csproj",
        ]);
        // Still a test for `tests/unchanged-test`, but no expectation.
        assert_eq!(
            head.role("src/Web/Handler.cs"),
            source(&["tests/Web.AcceptanceTests/HandlerTests.cs"], None)
        );
        assert_eq!(
            head.role("src/Application/Orders.cs"),
            source(&[], Some("OrdersTests.cs in Application.UnitTests"))
        );
    }

    #[test]
    fn other_languages_have_no_role() {
        let head = conventions(&["src/a.js", "README.md"]);
        assert_eq!(head.role("src/a.js"), Role::Other);
        assert_eq!(head.role("README.md"), Role::Other);
    }

    #[test]
    fn ancestors_end_at_the_root() {
        assert_eq!(ancestors("a/b").collect::<Vec<_>>(), ["a/b", "a", ""]);
        assert_eq!(ancestors("").collect::<Vec<_>>(), [""]);
    }

    const SERVICE: &str = "src/app/cart.service.ts";
    const SPEC: &str = "src/app/cart.service.spec.ts";

    /// Two versions of the file at `path` as its change, parsed by the
    /// path's extension.
    fn change(path: &str, before: &str, after: &str) -> FileChange {
        let language = if extension(path) == Some("cs") {
            Language::CSharp
        } else {
            Language::TypeScript
        };
        FileChange {
            path: PathBuf::from(path),
            old_path: None,
            before: parse(language, before).expect("before parses"),
            after: parse(language, after).expect("after parses"),
        }
    }

    macro_rules! fixture {
        ($path:expr, $case:literal, $ext:literal) => {
            change(
                $path,
                include_str!(concat!("../fixtures/tests/", $case, "/before.", $ext)),
                include_str!(concat!("../fixtures/tests/", $case, "/after.", $ext)),
            )
        };
    }

    fn none() -> PathPatterns {
        PathPatterns::new(&[]).expect("no patterns")
    }

    fn run(change: &FileChange, head: &[&str], changed: &[&str]) -> Vec<Finding> {
        run_excluding(change, head, changed, &none())
    }

    fn run_excluding(
        change: &FileChange,
        head: &[&str],
        changed: &[&str],
        exclude: &PathPatterns,
    ) -> Vec<Finding> {
        analyze(
            std::slice::from_ref(change),
            head.iter().copied(),
            changed.iter().copied(),
            exclude,
        )
    }

    fn warning(rule: &str, path: &str, line: u32, message: &str) -> Finding {
        Finding {
            rule: rule.to_owned(),
            severity: Severity::Warning,
            message: message.to_owned(),
            location: Location {
                path: PathBuf::from(path),
                line: Some(line),
                end_line: None,
            },
            metric: None,
            related: Vec::new(),
            hotspot: None,
        }
    }

    /// The head of a small Angular workspace with the service and its spec.
    const ANGULAR: &[&str] = &["package.json", SERVICE, SPEC];

    // test-analysis: "Unchanged test finding", "Service changed, spec not"
    #[test]
    fn service_changed_spec_not() {
        let service = fixture!(SERVICE, "service_changed", "ts");
        assert_eq!(
            run(&service, ANGULAR, &[SERVICE]),
            [warning(
                UNCHANGED_TEST_RULE,
                SERVICE,
                11,
                "CartService.total and 2 more changed, but not its test src/app/cart.service.spec.ts",
            )]
        );
    }

    // "The spec changed too", and a renamed spec counts as changed: the
    // caller lists it by its head path.
    #[test]
    fn the_spec_changed_too() {
        let service = fixture!(SERVICE, "service_changed", "ts");
        assert_eq!(run(&service, ANGULAR, &[SERVICE, SPEC]), []);
    }

    // "Excluded specs still count": the analyzed changes leave the spec
    // out, the changed paths do not.
    #[test]
    fn an_excluded_spec_still_counts() {
        let service = fixture!(SERVICE, "service_changed", "ts");
        let analyzed = [service];
        let findings = analyze(&analyzed, ANGULAR.iter().copied(), [SERVICE, SPEC], &none());
        assert_eq!(findings, []);
    }

    // "Several unchanged tests"
    #[test]
    fn several_unchanged_tests() {
        let service = fixture!(SERVICE, "service_changed", "ts");
        let head = [SERVICE, SPEC, "src/app/cart.service.test.ts"];
        let findings = run(&service, &head, &[SERVICE]);
        assert_eq!(
            findings[0].message,
            "CartService.total and 2 more changed, but not its test src/app/cart.service.spec.ts, and 1 more"
        );
    }

    // "One unit", in C#; "A C# overload"
    #[test]
    fn one_unit_and_a_csharp_overload() {
        let cart = fixture!("src/Domain/Cart.cs", "csharp_overload", "cs");
        let head = [
            "src/Domain/Domain.csproj",
            "src/Domain/Cart.cs",
            "tests/Domain.UnitTests/Domain.UnitTests.csproj",
            "tests/Domain.UnitTests/CartTests.cs",
        ];
        assert_eq!(
            run(&cart, &head, &["src/Domain/Cart.cs"]),
            [warning(
                UNCHANGED_TEST_RULE,
                "src/Domain/Cart.cs",
                14,
                "Cart.Total(int) changed, but not its test tests/Domain.UnitTests/CartTests.cs",
            )]
        );
    }

    // "One of several tests changed"
    #[test]
    fn one_of_several_tests_changed() {
        let path = "src/Application/TodoItems.cs";
        let todo = fixture!(path, "csharp_overload", "cs");
        let functional = "tests/Application.FunctionalTests/TodoItemsTests.cs";
        let head = [
            "src/Application/Application.csproj",
            path,
            "tests/Application.UnitTests/Application.UnitTests.csproj",
            "tests/Application.UnitTests/TodoItemsTests.cs",
            "tests/Application.FunctionalTests/Application.FunctionalTests.csproj",
            functional,
        ];
        assert_eq!(run(&todo, &head, &[path, functional]), []);
        assert_eq!(run(&todo, &head, &[path]).len(), 1);
    }

    // "A new function in a tested file"
    #[test]
    fn a_new_function_in_a_tested_file() {
        let service = fixture!(SERVICE, "new_function", "ts");
        let findings = run(&service, ANGULAR, &[SERVICE]);
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].message,
            "CartService.clear changed, but not its test src/app/cart.service.spec.ts"
        );
    }

    // "Changed units": none of these changes a unit.
    #[test]
    fn edits_that_change_no_unit() {
        for service in [
            fixture!(SERVICE, "layout_only", "ts"),
            fixture!(SERVICE, "type_only", "ts"),
            fixture!(SERVICE, "imports_and_types", "ts"),
            fixture!(SERVICE, "removed_function", "ts"),
        ] {
            assert_eq!(run(&service, ANGULAR, &[SERVICE]), [], "{service:?}");
        }
    }

    // "Renamed without edits": the base version is the old path's.
    #[test]
    fn renamed_without_edits() {
        let source = include_str!("../fixtures/tests/service_changed/before.ts");
        let mut renamed = change("src/app/basket.service.ts", source, source);
        renamed.old_path = Some(PathBuf::from(SERVICE));
        let head = [
            "src/app/basket.service.ts",
            "src/app/basket.service.spec.ts",
        ];
        assert_eq!(run(&renamed, &head, &["src/app/basket.service.ts"]), []);
    }

    // "Missing test finding", "New Angular service"
    #[test]
    fn new_angular_service() {
        let path = "src/app/orders/order.service.ts";
        let service = fixture!(path, "new_service", "ts");
        let head = ["package.json", "src/app/cart/cart.component.spec.ts", path];
        assert_eq!(
            run(&service, &head, &[path]),
            [warning(
                MISSING_TEST_RULE,
                path,
                8,
                "OrderService.load and 1 more have no test: expected src/app/orders/order.service.spec.ts",
            )]
        );
    }

    // "A new file with its spec"
    #[test]
    fn a_new_file_with_its_spec() {
        let path = "src/app/orders/order.service.ts";
        let service = fixture!(path, "new_service", "ts");
        let spec = "src/app/orders/order.service.spec.ts";
        assert_eq!(run(&service, &[path, spec], &[path, spec]), []);
    }

    // "New C# class", naming the first class test project by name
    #[test]
    fn new_csharp_class() {
        let path = "src/Domain/Invoice.cs";
        let invoice = fixture!(path, "csharp_new_class", "cs");
        let head = [
            "src/Domain/Domain.csproj",
            path,
            "tests/Domain.UnitTests/Domain.UnitTests.csproj",
            "tests/Domain.IntegrationTests/Domain.IntegrationTests.csproj",
            "tests/Domain.ArchitectureTests/Domain.ArchitectureTests.csproj",
        ];
        assert_eq!(
            run(&invoice, &head, &[path]),
            [warning(
                MISSING_TEST_RULE,
                path,
                7,
                "Invoice.Total() has no test: expected InvoiceTests.cs in Domain.ArchitectureTests",
            )]
        );
    }

    // "A new file with only types", "An existing file without a test",
    // "Repository without specs"
    #[test]
    fn what_a_missing_test_needs() {
        let models = change(
            "src/app/orders/order.model.ts",
            "",
            "export interface Order { id: string; }\n",
        );
        let head = ["package.json", "src/app/cart/cart.component.spec.ts"];
        assert_eq!(run(&models, &head, &["src/app/orders/order.model.ts"]), []);

        let legacy = fixture!("src/app/legacy/report.ts", "service_changed", "ts");
        assert_eq!(run(&legacy, &head, &["src/app/legacy/report.ts"]), []);

        let path = "src/format.ts";
        let new = fixture!(path, "new_service", "ts");
        assert_eq!(run(&new, &["package.json", path], &[path]), []);
    }

    // "Sources that need no test"
    #[test]
    fn tests_exclude() {
        let service = fixture!(SERVICE, "service_changed", "ts");
        let exclude = PathPatterns::new(&["src/app/**".to_owned()]).expect("valid");
        assert_eq!(run_excluding(&service, ANGULAR, &[SERVICE], &exclude), []);
        let everything = PathPatterns::new(&["**".to_owned()]).expect("valid");
        assert_eq!(
            run_excluding(&service, ANGULAR, &[SERVICE], &everything),
            []
        );
    }

    // "A changed spec alone", "A changed test project alone"
    #[test]
    fn tests_are_never_sources() {
        let spec = fixture!(SPEC, "service_changed", "ts");
        assert_eq!(run(&spec, ANGULAR, &[SPEC]), []);
        let path = "tests/Domain.UnitTests/OrderTests.cs";
        let test = fixture!(path, "csharp_overload", "cs");
        let head = ["tests/Domain.UnitTests/Domain.UnitTests.csproj", path];
        assert_eq!(run(&test, &head, &[path]), []);
    }
}
