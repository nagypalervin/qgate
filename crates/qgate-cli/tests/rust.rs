//! End-to-end tests for Rust in revision mode (`add-rust-analysis`): the
//! Rust scenarios of the `revision-input`, `complexity-analysis`,
//! `duplication-analysis`, `hotspot-analysis` and `test-analysis` specs.
//! Each test builds a throwaway git repository and runs the built binary
//! in it, as `csharp.rs` does.

mod support;

use std::fmt::Write as _;

use support::{Repo, stderr, stdout};

/// `Cart.total` with complexity 3, named on line 4.
const CART_BEFORE: &str = "pub struct Cart;

impl Cart {
    pub fn total(&self, d: u32) -> u32 {
        if d > 0 {
            return 1;
        }
        if d > 5 {
            return 2;
        }
        0
    }
}
";

/// The same method with a `match` of five arms and a final `_` arm:
/// complexity 8.
const CART_AFTER: &str = "pub struct Cart;

impl Cart {
    pub fn total(&self, d: u32) -> u32 {
        if d > 0 {
            return 1;
        }
        if d > 5 {
            return 2;
        }
        match d {
            6 => 6,
            7 => 7,
            8 => 8,
            9 => 9,
            10 => 10,
            _ => 0,
        }
    }
}
";

const CART_FINDING: &str = "error   crates/shop/src/cart.rs:4  Cart.total: cyclomatic complexity 3 -> 8 (+5)  \
                            [complexity/cyclomatic-increase]\n";

/// A repository with `CART_BEFORE` committed on `main` and `feature`
/// checked out, which then holds `files` committed.
fn change(files: &[(&str, &str)]) -> Repo {
    let repo = Repo::new();
    repo.write("crates/shop/src/cart.rs", CART_BEFORE);
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    for (path, contents) in files {
        repo.write(path, contents);
    }
    repo.commit("feature");
    repo
}

// complexity-analysis: "A Rust method that got more complex"; revision-input:
// "Rust files"
#[test]
fn a_rust_method_got_more_complex() {
    let repo = change(&[
        ("crates/shop/src/cart.rs", CART_AFTER),
        (
            "crates/shop/Cargo.toml",
            "[package]\nname = \"shop\"\nversion = \"0.1.0\"\n",
        ),
        ("Cargo.lock", "version = 4\n"),
    ]);
    let out = repo.qgate(&["--base", "main", "--fail-on", "never"]);
    assert_eq!(stdout(&out), CART_FINDING, "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn a_rust_change_in_the_working_tree() {
    let repo = Repo::new();
    repo.write("crates/shop/src/cart.rs", CART_BEFORE);
    repo.commit("base");
    repo.write("crates/shop/src/cart.rs", CART_AFTER);
    let out = repo.qgate(&["--base", "HEAD", "--worktree"]);
    assert_eq!(stdout(&out), CART_FINDING, "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(1));
}

// duplication-analysis: "Copy of an existing Rust file"
#[test]
fn copy_of_an_existing_rust_file() {
    let repo = Repo::new();
    repo.write(
        "crates/a/src/dates.rs",
        include_str!("../../qgate-analyzers/fixtures/duplication/date_helper.rs"),
    );
    repo.commit("base");
    repo.write(
        "crates/b/src/format.rs",
        include_str!("../../qgate-analyzers/fixtures/duplication/renamed_copy.rs"),
    );
    repo.commit("copy");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        "warning crates/b/src/format.rs:8  duplicates crates/a/src/dates.rs:6-14 \
         (lines 8-16, 67 tokens)  [duplication/duplicate-block]\n",
        "{}",
        stderr(&out)
    );
}

/// Sixteen calls that give the same 128 normalized tokens in TypeScript
/// and in Rust: `$id . mN ( $id , $lit )`.
fn calls() -> String {
    let mut calls = String::new();
    for n in 1..=16 {
        let _ = writeln!(calls, "    x.m{n}(a, \"s\");");
    }
    calls
}

// duplication-analysis: "The same tokens in two languages", with Rust
#[test]
fn copies_are_found_within_one_language_only() {
    let ts = calls();
    let rs = format!("fn f() {{\n{}}}\n", calls());
    let duplicates = |a: (&str, &str), b: (&str, &str)| {
        let repo = Repo::new();
        repo.write("README.md", "base\n");
        repo.commit("base");
        repo.write(a.0, a.1);
        repo.write(b.0, b.1);
        repo.commit("change");
        let out = repo.qgate(&["--base", "HEAD~1"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        stdout(&out).contains("[duplication/duplicate-block]")
    };
    assert!(duplicates(("src/a.rs", &rs), ("src/b.rs", &rs)));
    assert!(!duplicates(("src/a.ts", &ts), ("src/b.rs", &rs)));
}

/// A `Walker` whose `step` has complexity 11, in an `impl` block written
/// as `header`.
fn walker(header: &str) -> String {
    let mut ifs = String::new();
    for i in 0..10 {
        let _ = writeln!(
            ifs,
            "        if self.1 > {i} {{\n            self.1 -= 1;\n        }}"
        );
    }
    format!(
        "pub struct Walker<'s>(&'s str, u32);\n\n{header} {{\n    fn step(&mut self) {{\n{ifs}    }}\n}}\n"
    )
}

// rust-analysis: "A lifetime renamed" (`rust-names-without-lifetimes`).
// With lifetimes in the name, `Walker<'_>.step` was a new function of
// complexity 11, a Warning.
#[test]
fn a_lifetime_renamed_is_the_same_unit() {
    let repo = Repo::new();
    repo.write("crates/lang/src/walker.rs", walker("impl<'s> Walker<'s>"));
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("crates/lang/src/walker.rs", walker("impl Walker<'_>"));
    repo.commit("feature");
    let out = repo.qgate(&["--base", "main", "--fail-on", "warning"]);
    assert_eq!(stdout(&out), "No regressions found.\n", "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
}

/// A Rust file at revision `rev` whose `run` has `decisions` `if`s.
fn rust_file(rev: usize, decisions: usize) -> String {
    let mut ifs = String::new();
    for i in 0..decisions {
        let _ = writeln!(ifs, "    if d > {i} {{\n        d -= 1;\n    }}");
    }
    format!("// {rev}\nfn run(mut d: u32) -> u32 {{\n{ifs}    d\n}}\n")
}

// hotspot-analysis: "Rust files": an integration test is ranked among the
// Rust files, so a finding in it gets hotspot context.
#[test]
fn rust_integration_tests_are_ranked() {
    let repo = Repo::new();
    repo.write("crates/cli/src/main.rs", rust_file(0, 0));
    repo.write("crates/cli/src/lib.rs", rust_file(0, 1));
    // A busier, more complex TypeScript file: ranked with the Rust files,
    // it would push `check.rs` out of the top 10% (review of PR 2).
    let mut ts = String::new();
    for i in 0..20 {
        let _ = writeln!(ts, "  if (d > {i}) {{ d--; }}");
    }
    for rev in 0..9 {
        if rev < 6 {
            repo.write("crates/cli/tests/check.rs", rust_file(rev, 8));
        }
        repo.write(
            "web/busy.ts",
            format!("// {rev}\nexport function busy(d: number) {{\n{ts}  return d;\n}}\n"),
        );
        repo.commit_at("check", &format!("2026-01-{:02}T00:00:00Z", rev + 1));
    }
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("crates/cli/tests/check.rs", rust_file(99, 10));
    repo.commit_at("raise", "2026-03-01T00:00:00Z");
    let out = stdout(&repo.qgate(&["--base", "main"]));
    assert!(
        out.contains(
            "run: cyclomatic complexity 9 -> 11 (+2); hotspot: 7 commits in 6 months, complexity 11"
        ),
        "{out}"
    );
}

// test-analysis: "Rust files"
#[test]
fn rust_files_get_no_test_findings() {
    let repo = Repo::new();
    repo.write("crates/a/src/lib.rs", rust_file(0, 1));
    repo.write("crates/a/tests/it.rs", rust_file(0, 0));
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write(
        "crates/shop/src/cart.rs",
        "pub fn add(a: u32) -> u32 {\n    a + 1\n}\n\npub fn sub(a: u32) -> u32 {\n    a - 1\n}\n",
    );
    repo.write("crates/a/src/lib.rs", rust_file(1, 2));
    repo.commit("change");
    let out = repo.qgate(&["--base", "main"]);
    // The edited `run` got more complex; that is its only finding.
    assert_eq!(
        stdout(&out),
        "warning crates/a/src/lib.rs:2  run: cyclomatic complexity 2 -> 3 (+1)  \
         [complexity/cyclomatic-increase]\n",
        "{}",
        stderr(&out)
    );
}
