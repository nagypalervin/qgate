//! End-to-end tests for hotspot context: the scenarios of the
//! `hotspot-analysis` spec that run `qgate check`, and "A finding in a
//! hotspot" of `report-formats` (`add-hotspot-context`). Each test builds
//! a throwaway git repository with dated commits and runs the built
//! binary in it.

mod support;

use std::fmt::Write as _;

use support::{Repo, function, qgate_in, stderr, stdout};

/// `src/cart.ts` at revision `rev`: a comment, then `total` with
/// `decisions` decision points (line 2) and `add` with 3. Its complexity
/// is `decisions + 1 + 4`.
fn cart(rev: usize, decisions: usize) -> String {
    format!(
        "// rev {rev}\n{}{}",
        function("total", decisions),
        function("add", 3)
    )
}

fn date(day: usize) -> String {
    format!("2026-01-{day:02}T00:00:00Z")
}

/// The date of the feature branch's commit.
const FEATURE: &str = "2026-03-01T00:00:00Z";

/// `main`: `src/cart.ts` (complexity 9) changed in `commits` commits from
/// 2026-01-01, `src/a.ts` and `src/b.ts` (complexity 1 each) in the
/// first. The repository is left on a new branch `feature`.
fn busy_cart(commits: usize) -> Repo {
    let repo = Repo::new();
    repo.write("src/a.ts", function("a", 0));
    repo.write("src/b.ts", function("b", 0));
    for rev in 0..commits {
        repo.write("src/cart.ts", cart(rev, 4));
        repo.commit_at("cart", &date(rev + 1));
    }
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo
}

/// Commit `total` at 6 decision points (complexity 5 -> 7) on `feature`
/// and run `qgate check --base main` with `args`.
fn raise_total(repo: &Repo, args: &[&str]) -> (String, String, Option<i32>) {
    repo.write("src/cart.ts", cart(99, 6));
    repo.commit_at("raise total", FEATURE);
    let mut all = vec!["--base", "main"];
    all.extend_from_slice(args);
    let out = repo.qgate(&all);
    (stdout(&out), stderr(&out), out.status.code())
}

const NOTE: &str = "; hotspot: 7 commits in 6 months, complexity 11";

// "A complexity increase in a hotspot"
#[test]
fn a_complexity_increase_in_a_hotspot() {
    let repo = busy_cart(6);
    let (out, err, code) = raise_total(&repo, &[]);
    assert_eq!(
        out,
        format!(
            "warning src/cart.ts:2  total: cyclomatic complexity 5 -> 7 (+2){NOTE}  [complexity/cyclomatic-increase]\n"
        ),
        "{err}"
    );
    assert_eq!(err, "");
    assert_eq!(code, Some(0));
}

// "The hotspot note" in every format, and report-formats "A finding in a
// hotspot"
#[test]
fn the_note_in_every_format() {
    let repo = busy_cart(6);
    raise_total(&repo, &[]);
    let run = |format: &str| stdout(&repo.qgate(&["--base", "main", "--format", format]));
    let markdown = run("markdown");
    // Markdown escapes `>` ("Markdown table cells" in `report-formats`).
    assert!(
        markdown.contains(&format!("total: cyclomatic complexity 5 -\\> 7 (+2){NOTE}")),
        "{markdown}"
    );
    let azure = run("azure");
    assert!(
        azure.contains(&format!("cyclomatic complexity 5 -> 7 (+2){NOTE}")),
        "{azure}"
    );
    let github = run("github");
    assert!(github.contains(NOTE), "{github}");
    let json = run("json");
    assert!(
        json.contains(&format!(
            "\"message\":\"total: cyclomatic complexity 5 -> 7 (+2){NOTE}\",\"path\":\"src/cart.ts\",\"line\":2,\"metric\":{{\"before\":5,\"after\":7}},\"hotspot\":{{\"commits\":7,\"complexity\":11}}}}"
        )),
        "{json}"
    );
}

// "A hotspot nothing made worse"
#[test]
fn a_hotspot_nothing_made_worse() {
    let repo = busy_cart(6);
    repo.write("src/cart.ts", cart(99, 4));
    repo.commit_at("comment only", FEATURE);
    let out = repo.qgate(&["--base", "main"]);
    assert_eq!(stdout(&out), "No regressions found.\n");
}

// "An unparsable hotspot"
#[test]
fn an_unparsable_hotspot() {
    let repo = busy_cart(6);
    repo.write(
        "src/cart.ts",
        format!("{}function broken( {{\n", cart(99, 4)),
    );
    repo.commit_at("break it", FEATURE);
    let out = stdout(&repo.qgate(&["--base", "main"]));
    assert!(out.contains("[input/unparsable]"), "{out}");
    assert!(!out.contains("hotspot"), "{out}");
}

// "Two-file form"
#[test]
fn two_file_form() {
    let repo = busy_cart(6);
    repo.write("before.ts", cart(0, 4));
    repo.write("src/cart.ts", cart(99, 6));
    let out = stdout(&qgate_in(&repo.dir, &["before.ts", "src/cart.ts"]));
    assert!(out.contains("[complexity/cyclomatic-increase]"), "{out}");
    assert!(!out.contains("hotspot"), "{out}");
}

// "The window": commits more than 182 days before the head do not count.
#[test]
fn old_commits_are_outside_the_window() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("a", 0));
    for rev in 0..4 {
        repo.write("src/cart.ts", cart(rev, 4));
        repo.commit_at("old", &format!("2025-0{}-01T00:00:00Z", rev + 1));
    }
    repo.write("src/cart.ts", cart(5, 4));
    repo.commit_at("recent", &date(1));
    repo.git(&["checkout", "-q", "-b", "feature"]);
    let (out, _, _) = raise_total(&repo, &[]);
    // Only the recent commit and the feature's are in the window.
    assert!(!out.contains("hotspot"), "{out}");
}

// "Churn": merge commits do not count, the commits they bring in do.
#[test]
fn merge_commits() {
    let repo = busy_cart(3);
    repo.git(&["checkout", "-q", "-b", "side"]);
    for rev in 10..13 {
        repo.write("src/cart.ts", cart(rev, 4));
        repo.commit_at("side", &date(rev));
    }
    repo.git(&["checkout", "-q", "feature"]);
    repo.write("src/a.ts", function("a2", 0));
    repo.commit_at("feature moves", &date(20));
    repo.merge_at("side", &date(21));
    let (out, _, _) = raise_total(&repo, &[]);
    // 3 on main, 3 on side, and the feature's commit: 7, not 8.
    assert!(out.contains(NOTE), "{out}");
}

// "Churn": "Renamed in the window"
#[test]
fn a_rename_in_the_window() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("a", 0));
    for rev in 0..4 {
        repo.write("src/basket.ts", cart(rev, 4));
        repo.commit_at("basket", &date(rev + 1));
    }
    repo.git(&["mv", "src/basket.ts", "src/cart.ts"]);
    repo.commit_at("rename", &date(10));
    repo.write("src/cart.ts", cart(5, 4));
    repo.commit_at("cart", &date(11));
    repo.git(&["checkout", "-q", "-b", "feature"]);
    let (out, _, _) = raise_total(&repo, &[]);
    assert!(out.contains(NOTE), "{out}");
}

// "Ranked files": "Excluded migrations"
#[test]
fn excluded_files_are_not_ranked() {
    let build = |exclude: bool| {
        let repo = Repo::new();
        if exclude {
            repo.write("qgate.toml", "[files]\nexclude = [\"**/Migrations/**\"]\n");
        }
        for i in 0..9 {
            repo.write(&format!("src/f{i}.ts"), function(&format!("f{i}"), 0));
        }
        for rev in 0..8 {
            if rev < 6 {
                repo.write("src/cart.ts", cart(rev, 4));
            }
            for m in 0..2 {
                repo.write(
                    &format!("db/Migrations/m{m}.ts"),
                    format!("// {rev}\n{}", function(&format!("m{m}"), 9)),
                );
            }
            repo.commit_at("cart and migrations", &date(rev + 1));
        }
        repo.git(&["checkout", "-q", "-b", "feature"]);
        raise_total(&repo, &[]).0
    };
    // Two busier (8 commits against the cart's 7), more complex
    // migrations among 12 files push the cart out of the top 10% ...
    assert!(!build(false).contains("hotspot"));
    // ... unless they are excluded.
    let out = build(true);
    assert!(out.contains(NOTE), "{out}");
}

// "Ranked files": "Families apart"
#[test]
fn families_are_ranked_apart() {
    let repo = busy_cart(6);
    repo.git(&["checkout", "-q", "main"]);
    let mut ifs = String::new();
    for i in 0..30 {
        let _ = writeln!(ifs, "        if (d > {i}) {{ d--; }}");
    }
    for rev in 0..9 {
        repo.write(
            "src/Big.cs",
            format!(
                "// {rev}\nnamespace Shop;\n\npublic class Big\n{{\n    public int Run(int d)\n    {{\n{ifs}        return d;\n    }}\n}}\n"
            ),
        );
        repo.commit_at("big", &date(rev + 10));
    }
    repo.git(&["checkout", "-q", "feature"]);
    repo.git_at(&["merge", "-q", "--no-ff", "-m", "main", "main"], &date(25));
    let (out, _, _) = raise_total(&repo, &[]);
    // A busier, more complex C# file does not push the TypeScript one out.
    assert!(out.contains(NOTE), "{out}");
}

// "Shallow history"
#[test]
fn a_shallow_clone() {
    let repo = busy_cart(6);
    raise_total(&repo, &[]);
    let clone = Repo::new();
    let url = format!("file://{}", repo.dir.display());
    clone.git(&[
        "fetch",
        "-q",
        "--depth",
        "3",
        &url,
        "feature:feature",
        "main:refs/remotes/origin/main",
    ]);
    clone.git(&["checkout", "-q", "feature"]);
    let out = clone.qgate(&["--base", "origin/main"]);
    assert_eq!(
        stdout(&out),
        "warning src/cart.ts:2  total: cyclomatic complexity 5 -> 7 (+2)  [complexity/cyclomatic-increase]\n"
    );
    assert_eq!(
        stderr(&out),
        "qgate: the history is shallow; hotspots not computed\n"
    );
    assert_eq!(out.status.code(), Some(0));
}

// "The window" with `--worktree`: the snapshot is not history.
#[test]
fn the_working_tree() {
    let repo = busy_cart(7);
    repo.write("src/cart.ts", cart(99, 6));
    let out = stdout(&repo.qgate(&["--base", "main", "--worktree"]));
    // The 7 commits of history; the uncommitted edit is no commit.
    assert!(out.contains(NOTE), "{out}");
}
