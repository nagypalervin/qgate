//! End-to-end tests for architecture rules on C# `using` directives
//! (`add-csharp-architecture`): the C# scenarios of the
//! `architecture-analysis` and `configuration` specs. Each test builds a
//! throwaway git repository and runs the built binary in it.

mod support;

use support::{Repo, stderr, stdout};

/// The domain must not use the infrastructure or EF Core.
const RULE: &str = "[[architecture.forbid]]\nname = \"domain-is-pure\"\n\
                    from = [\"src/Shop.Domain/**\"]\n\
                    to_namespaces = [\"Shop.Infrastructure\", \"Microsoft.EntityFrameworkCore\"]\n";

/// A domain class with `usings` above its namespace.
fn order(usings: &str) -> String {
    format!(
        "{usings}\nnamespace Shop.Domain;\n\npublic class Order\n{{\n    public decimal Total {{ get; set; }}\n}}\n"
    )
}

/// Commit `base` on `main`, then `change` on `feature`, and run
/// `--base main`.
fn run(base: &[(&str, &str)], change: &[(&str, &str)]) -> (String, String, Option<i32>) {
    let repo = Repo::new();
    repo.write("README.md", "base\n");
    for (path, contents) in base {
        repo.write(path, contents);
    }
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    for (path, contents) in change {
        repo.write(path, contents);
    }
    repo.commit("change");
    let out = repo.qgate(&["--base", "main"]);
    (stdout(&out), stderr(&out), out.status.code())
}

/// [`run`] with [`RULE`] and `src/Shop.Domain/Order.cs` going from
/// `before` to `after`.
fn run_order(before: &str, after: &str) -> (String, String, Option<i32>) {
    run(
        &[("qgate.toml", RULE), ("src/Shop.Domain/Order.cs", before)],
        &[("src/Shop.Domain/Order.cs", after)],
    )
}

fn error_line(path_line: &str, name: &str, rule: &str) -> String {
    format!(
        "error   {path_line}  uses {name}, forbidden by \"{rule}\"  [architecture/forbidden-import]\n"
    )
}

// "Domain using the infrastructure", and "Only namespaces" (a rule
// without `to` is valid).
#[test]
fn domain_using_the_infrastructure() {
    let (out, err, code) = run_order(
        &order("using System;"),
        &order("using System;\nusing Shop.Infrastructure.Data;"),
    );
    assert_eq!(
        out,
        error_line(
            "src/Shop.Domain/Order.cs:2",
            "Shop.Infrastructure.Data",
            "domain-is-pure"
        ),
        "{err}"
    );
    assert_eq!(code, Some(1));
}

// "Each list applies to its language"
#[test]
fn each_list_applies_to_its_language() {
    let config = "[[architecture.forbid]]\nname = \"imports\"\nfrom = [\"src/**\"]\n\
                  to = [\"Shop.Infrastructure\"]\n\n\
                  [[architecture.forbid]]\nname = \"usings\"\nfrom = [\"src/**\"]\n\
                  to_namespaces = [\"Shop.Infrastructure\"]\n";
    let (out, err, _) = run(
        &[("qgate.toml", config)],
        &[
            ("src/a.ts", "import \"Shop.Infrastructure\";\n"),
            ("src/B.cs", "using Shop.Infrastructure;\n\nclass B { }\n"),
        ],
    );
    assert_eq!(
        out,
        format!(
            "{}{}",
            error_line("src/B.cs:1", "Shop.Infrastructure", "usings"),
            "error   src/a.ts:1  imports Shop.Infrastructure, forbidden by \"imports\"  \
             [architecture/forbidden-import]\n"
        ),
        "{err}"
    );
}

// "Existing using, written another way"
#[test]
fn existing_using_written_another_way() {
    let mut after = order("global using global::Shop.Infrastructure.Data;");
    after.push_str("\npublic class Line\n{\n}\n");
    let (out, err, code) = run_order(&order("using Shop.Infrastructure.Data;"), &after);
    assert_eq!(out, "No regressions found.\n", "{err}");
    assert_eq!(code, Some(0));
}

// "Same namespace used twice"
#[test]
fn same_namespace_used_twice() {
    let (out, err, _) = run_order(
        &order(""),
        &order(
            "using Microsoft.EntityFrameworkCore;\nusing System;\n\nusing M = Microsoft.EntityFrameworkCore;",
        ),
    );
    assert_eq!(
        out,
        error_line(
            "src/Shop.Domain/Order.cs:1",
            "Microsoft.EntityFrameworkCore",
            "domain-is-pure"
        ),
        "{err}"
    );
}

// "Every directive form"
#[test]
fn every_directive_form() {
    let (out, err, _) = run_order(
        &order(""),
        &order(
            "using Shop.Infrastructure.A;\nglobal using Shop.Infrastructure.B;\n\
             using static Shop.Infrastructure.C;\nusing D = Shop.Infrastructure.D;",
        ),
    );
    let expected: String = [("1", "A"), ("2", "B"), ("3", "C"), ("4", "D")]
        .iter()
        .map(|(line, name)| {
            error_line(
                &format!("src/Shop.Domain/Order.cs:{line}"),
                &format!("Shop.Infrastructure.{name}"),
                "domain-is-pure",
            )
        })
        .collect();
    assert_eq!(out, expected, "{err}");
}

// "Inside a namespace and an `#if`"
#[test]
fn inside_a_namespace_and_an_if() {
    let after = "#if DEBUG\nusing Shop.Infrastructure.Debug;\n#endif\n\n\
                 namespace Shop.Domain\n{\n    using Shop.Infrastructure.Data;\n\n    \
                 public class Order\n    {\n    }\n}\n";
    let before = "namespace Shop.Domain\n{\n    public class Order\n    {\n    }\n}\n";
    let (out, err, _) = run_order(before, after);
    assert_eq!(
        out,
        format!(
            "{}{}",
            error_line(
                "src/Shop.Domain/Order.cs:2",
                "Shop.Infrastructure.Debug",
                "domain-is-pure"
            ),
            error_line(
                "src/Shop.Domain/Order.cs:7",
                "Shop.Infrastructure.Data",
                "domain-is-pure"
            ),
        ),
        "{err}"
    );
}

// "Using statements"
#[test]
fn using_statements() {
    let after = "namespace Shop.Domain;\n\npublic class Order\n{\n    public void Save()\n    {\n        \
                 using var db = new Shop.Infrastructure.Db();\n        using (db) { }\n    }\n}\n";
    let (out, err, code) = run_order(&order(""), after);
    assert_eq!(out, "No regressions found.\n", "{err}");
    assert_eq!(code, Some(0));
}

// "As written inside a namespace"
#[test]
fn as_written_inside_a_namespace() {
    let before = "namespace Shop\n{\n    public class Order\n    {\n    }\n}\n";
    let after =
        "namespace Shop\n{\n    using Infrastructure;\n\n    public class Order\n    {\n    }\n}\n";
    let (out, err, _) = run_order(before, after);
    assert_eq!(out, "No regressions found.\n", "{err}");
}

// "Empty segment"
#[test]
fn invalid_namespace_pattern() {
    let config = "[[architecture.forbid]]\nname = \"domain-is-pure\"\nfrom = [\"src/**\"]\n\
                  to_namespaces = [\"Shop..Infrastructure\"]\n";
    let (out, err, code) = run(
        &[],
        &[("qgate.toml", config), ("src/A.cs", "class A { }\n")],
    );
    assert_eq!(out, "");
    assert!(err.contains("qgate.toml"), "{err}");
    assert!(err.contains("\"domain-is-pure\""), "{err}");
    assert!(err.contains("\"Shop..Infrastructure\""), "{err}");
    assert_eq!(code, Some(2));
}

// "Rule without targets"
#[test]
fn rule_without_targets() {
    let config = "[[architecture.forbid]]\nname = \"domain-is-pure\"\nfrom = [\"src/**\"]\n";
    let (out, err, code) = run(
        &[],
        &[("qgate.toml", config), ("src/A.cs", "class A { }\n")],
    );
    assert_eq!(out, "");
    assert!(err.contains("qgate.toml"), "{err}");
    assert!(err.contains("\"domain-is-pure\""), "{err}");
    assert!(err.contains("`to`"), "{err}");
    assert!(err.contains("`to_namespaces`"), "{err}");
    assert_eq!(code, Some(2));
}

// Generated and excluded C# files take no part (design D6).
#[test]
fn generated_and_excluded_files_are_not_checked() {
    let config = format!("{RULE}\n[files]\nexclude = [\"src/Shop.Domain/Legacy/**\"]\n");
    let using = order("using Shop.Infrastructure.Data;");
    let header = format!("// <auto-generated />\n{using}");
    let (out, err, code) = run(
        &[("qgate.toml", &config)],
        &[
            ("src/Shop.Domain/Order.g.cs", &using),
            ("src/Shop.Domain/Client.cs", &header),
            ("src/Shop.Domain/Legacy/Order.cs", &using),
        ],
    );
    assert_eq!(out, "No regressions found.\n", "{err}");
    assert_eq!(code, Some(0));
    // The same file under a checked name is reported.
    let (out, _, _) = run(
        &[("qgate.toml", &config)],
        &[("src/Shop.Domain/Order.cs", &using)],
    );
    assert!(out.contains("uses Shop.Infrastructure.Data"), "{out}");
}
