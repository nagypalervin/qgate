//! End-to-end tests for `[files] exclude` in `qgate.toml`: the scenarios
//! of "Excluded paths" in the `configuration` spec (`add-path-exclusion`).
//! Each test builds a throwaway git repository and runs the built binary
//! in it.

mod support;

use std::fmt::Write as _;

use support::{DATE_HELPER, RENAMED_COPY, Repo, function, stderr, stdout};

/// A C# class whose one method has complexity `1 + decisions`.
fn cs_class(name: &str, decisions: usize) -> String {
    let mut ifs = String::new();
    for i in 0..decisions {
        let _ = writeln!(ifs, "        if (d > {i}) {{ d--; }}");
    }
    format!(
        "namespace Shop;\n\npublic class {name}\n{{\n    public int Run(int d)\n    {{\n{ifs}        return d;\n    }}\n}}\n"
    )
}

/// An EF Core-style migration: the same model in every migration.
fn migration(name: &str) -> String {
    let mut model = String::new();
    for table in ["Orders", "Lines", "Customers"] {
        let _ = write!(
            model,
            "        migrationBuilder.CreateTable(name: \"{table}\", columns: table => new\n        {{\n            \
             Id = table.Column<int>(type: \"int\", nullable: false),\n            \
             Name = table.Column<string>(type: \"nvarchar(200)\", nullable: true)\n        }});\n"
        );
    }
    format!(
        "namespace Shop.Data.Migrations;\n\npublic partial class {name} : Migration\n{{\n    \
         protected override void Up(MigrationBuilder migrationBuilder)\n    {{\n{model}    }}\n}}\n"
    )
}

const EXCLUDE_MIGRATIONS: &str = "[files]\nexclude = [\"**/Migrations/**\"]\n";

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

// "Migrations left out"
#[test]
fn migrations_left_out() {
    let mut added = migration("AddLines");
    added.push_str(&cs_class("Seed", 24));
    let base = [("src/Data/Migrations/20261101_Init.cs", migration("Init"))];
    let base: Vec<(&str, &str)> = base.iter().map(|(p, c)| (*p, c.as_str())).collect();
    // Without the setting: a copy and a complex new method.
    let (out, err, _) = run(
        &base,
        &[("src/Data/Migrations/20261120_AddLines.cs", &added)],
    );
    assert!(out.contains("[duplication/duplicate-block]"), "{out}{err}");
    assert!(out.contains("[complexity/cyclomatic-new]"), "{out}{err}");
    // With it: nothing.
    let (out, err, code) = run(
        &base,
        &[
            ("qgate.toml", EXCLUDE_MIGRATIONS),
            ("src/Data/Migrations/20261120_AddLines.cs", &added),
        ],
    );
    assert_eq!(out, "No regressions found.\n", "{err}");
    assert_eq!(code, Some(0));
}

// "An excluded file is no original"
#[test]
fn an_excluded_file_is_no_original() {
    let base = [
        ("qgate.toml", "[files]\nexclude = [\"vendor\"]\n"),
        ("vendor/date.ts", DATE_HELPER),
    ];
    let (out, err, _) = run(&base, &[("src/orders/format.ts", RENAMED_COPY)]);
    assert_eq!(out, "No regressions found.\n", "{err}");
    // The same copy of an included file is reported.
    let (out, _, _) = run(
        &[("src/utils/date.ts", DATE_HELPER)],
        &[("src/orders/format.ts", RENAMED_COPY)],
    );
    assert!(out.contains("[duplication/duplicate-block]"), "{out}");
}

// "Renamed out of an excluded directory"
#[test]
fn renamed_out_of_an_excluded_directory_is_new() {
    let repo = Repo::new();
    repo.write("qgate.toml", "[files]\nexclude = [\"legacy/**\"]\n");
    repo.write("legacy/Report.cs", cs_class("Report", 14));
    repo.write("src/README.md", "sources\n");
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.git(&["mv", "legacy/Report.cs", "src/Report.cs"]);
    repo.commit("rename");
    let out = repo.qgate(&["--base", "main"]);
    assert_eq!(
        stdout(&out),
        "warning src/Report.cs:5  Report.Run(int): new function, cyclomatic complexity 15  \
         [complexity/cyclomatic-new]\n",
        "{}",
        stderr(&out)
    );
}

// An excluded JavaScript file renamed to TypeScript is no base either
// (design D3).
#[test]
fn an_excluded_javascript_base_is_no_base() {
    let repo = Repo::new();
    repo.write("qgate.toml", "[files]\nexclude = [\"vendor\"]\n");
    repo.write("vendor/date.js", DATE_HELPER);
    repo.write("src/utils/date.ts", DATE_HELPER);
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.git(&["mv", "vendor/date.js", "src/date.ts"]);
    repo.commit("rename");
    let out = repo.qgate(&["--base", "main"]);
    assert!(
        stdout(&out).contains("src/date.ts:1  duplicates src/utils/date.ts:1-6"),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
}

// "Excluded in the working tree"
#[test]
fn excluded_in_the_working_tree() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("a", 1));
    repo.commit("base");
    repo.write("qgate.toml", "[files]\nexclude = [\"generated/**\"]\n");
    repo.write("generated/Client.ts", function("client", 24));
    let out = repo.qgate(&["--base", "HEAD", "--worktree"]);
    assert_eq!(stdout(&out), "No regressions found.\n", "{}", stderr(&out));
    // Without the setting the function is reported.
    repo.write("qgate.toml", "");
    let out = repo.qgate(&["--base", "HEAD", "--worktree", "--fail-on", "never"]);
    assert!(
        stdout(&out).contains("generated/Client.ts"),
        "{}",
        stdout(&out)
    );
}

// "Invalid exclude pattern"
#[test]
fn invalid_exclude_pattern() {
    let (out, err, code) = run(
        &[],
        &[
            ("qgate.toml", "[files]\nexclude = [\"/src/**\"]\n"),
            ("src/a.ts", &function("a", 1)),
        ],
    );
    assert_eq!(out, "");
    assert!(err.contains("qgate.toml"), "{err}");
    assert!(err.contains("files.exclude"), "{err}");
    assert!(err.contains("\"/src/**\""), "{err}");
    assert_eq!(code, Some(2));
}

// Out of scope of exclusion (proposal): a rule's `to` still matches an
// excluded target.
#[test]
fn an_excluded_file_is_still_an_import_target() {
    let config = "[files]\nexclude = [\"src/ui\"]\n\n[[architecture.forbid]]\nname = \"domain-is-pure\"\n\
                  from = [\"src/domain/**\"]\nto = [\"src/ui/**\"]\n";
    let (out, err, _) = run(
        &[
            ("qgate.toml", config),
            ("src/ui/Button.ts", "export const B = 1;\n"),
        ],
        &[(
            "src/domain/order.ts",
            "import { B } from \"../ui/Button\";\n",
        )],
    );
    assert!(
        out.contains("src/domain/order.ts:1  imports src/ui/Button"),
        "{out}{err}"
    );
}

/// `contents` with enough other code that git sees an added file rather
/// than a rename of a deleted one.
fn with_other_code(contents: &str) -> String {
    let mut text = String::from(contents);
    for i in 0..12 {
        text.push_str(&function(&format!("other{i}"), 1));
    }
    text
}

// "Excluded paths": a deleted excluded file, TypeScript or JavaScript, is
// no code that existed before (design D3).
#[test]
fn a_deleted_excluded_file_is_no_earlier_code() {
    for vendor in ["vendor/date.ts", "vendor/date.js"] {
        let repo = Repo::new();
        repo.write("qgate.toml", "[files]\nexclude = [\"vendor\"]\n");
        repo.write(vendor, DATE_HELPER);
        repo.write("src/utils/date.ts", DATE_HELPER);
        repo.commit("base");
        repo.git(&["checkout", "-q", "-b", "feature"]);
        repo.git(&["rm", "-q", vendor]);
        repo.write("src/orders/format.ts", with_other_code(RENAMED_COPY));
        repo.commit("change");
        let out = repo.qgate(&["--base", "main"]);
        assert!(
            stdout(&out).contains("src/orders/format.ts:4  duplicates src/utils/date.ts:1-6"),
            "{vendor}: {}{}",
            stdout(&out),
            stderr(&out)
        );
    }
}

// A file renamed into an excluded directory: its old code left, so a
// copy of it elsewhere is no new copy (design D3).
#[test]
fn a_file_renamed_into_an_excluded_directory_keeps_its_old_code() {
    let repo = Repo::new();
    repo.write("qgate.toml", "[files]\nexclude = [\"vendor\"]\n");
    repo.write("src/a.ts", DATE_HELPER);
    repo.write("vendor/README.md", "vendored\n");
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.git(&["mv", "src/a.ts", "vendor/a.ts"]);
    repo.write("src/b.ts", RENAMED_COPY);
    repo.commit("move");
    let out = repo.qgate(&["--base", "main"]);
    assert_eq!(stdout(&out), "No regressions found.\n", "{}", stderr(&out));
}
