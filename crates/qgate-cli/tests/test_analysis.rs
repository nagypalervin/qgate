//! End-to-end tests for the tests analyzer: the scenarios of the
//! `test-analysis` spec and of "The tests table" in `configuration`
//! (`add-test-gap-analysis`). Each test builds a throwaway git repository
//! and runs the built binary in it.

mod support;

use std::fmt::Write as _;

use support::{Repo, function, qgate_in, stderr, stdout};

/// `CartService` with three methods, `total` on line 4; `edit` changes
/// all three bodies.
fn cart_service(edit: bool) -> String {
    let d = if edit { " + 1" } else { "" };
    format!(
        "import {{ Item }} from \"./item\";\n\nexport class CartService {{\n  total(): number {{\n    return 1{d};\n  }}\n\n  add(): number {{\n    return 2{d};\n  }}\n\n  remove(): number {{\n    return 3{d};\n  }}\n}}\n"
    )
}

/// A spec; `version` tells two versions apart.
fn spec(version: u32) -> String {
    format!("describe(\"CartService\", () => {{\n  it(\"works {version}\", () => {{}});\n}});\n")
}

const SERVICE: &str = "src/app/cart.service.ts";
const SPEC: &str = "src/app/cart.service.spec.ts";
const CSPROJ: &str = "<Project Sdk=\"Microsoft.NET.Sdk\" />\n";

/// A C# class `name` with one method `Ship()`, on line 5, returning
/// `value`.
fn cs_class(name: &str, value: u32) -> String {
    format!(
        "namespace Shop;\n\npublic class {name}\n{{\n    public int Ship()\n    {{\n        return {value};\n    }}\n}}\n"
    )
}

/// Commit `base` on `main`, then `change` on `feature`, and run
/// `--base main` with `args`. A path with empty contents is deleted.
fn run_with(
    base: &[(&str, &str)],
    change: &[(&str, &str)],
    args: &[&str],
) -> (String, String, Option<i32>) {
    let repo = Repo::new();
    repo.write("README.md", "base\n");
    for (path, contents) in base {
        repo.write(path, contents);
    }
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    for (path, contents) in change {
        if contents.is_empty() {
            repo.remove(path);
        } else {
            repo.write(path, contents);
        }
    }
    repo.commit("change");
    let mut all = vec!["--base", "main"];
    all.extend_from_slice(args);
    let out = repo.qgate(&all);
    (stdout(&out), stderr(&out), out.status.code())
}

fn run(base: &[(&str, &str)], change: &[(&str, &str)]) -> (String, String, Option<i32>) {
    run_with(base, change, &[])
}

/// The output's `tests/` lines.
fn test_lines(out: &str) -> Vec<&str> {
    out.lines().filter(|l| l.contains("[tests/")).collect()
}

/// An Angular workspace with the service and its spec.
fn angular() -> Vec<(&'static str, String)> {
    vec![
        ("package.json", "{}\n".to_owned()),
        (SERVICE, cart_service(false)),
        (SPEC, spec(1)),
    ]
}

fn refs<'a>(files: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    files.iter().map(|(p, c)| (*p, c.as_str())).collect()
}

// "Unchanged test finding": "Service changed, spec not"
#[test]
fn service_changed_spec_not() {
    let base = angular();
    let edited = cart_service(true);
    let (out, err, code) = run(&refs(&base), &[(SERVICE, &edited)]);
    assert_eq!(
        test_lines(&out),
        [
            "warning src/app/cart.service.ts:4  CartService.total and 2 more changed, but not its test src/app/cart.service.spec.ts  [tests/unchanged-test]"
        ],
        "{out}{err}"
    );
    assert_eq!(code, Some(0));
}

// "Failing on warnings"
#[test]
fn failing_on_warnings() {
    let base = angular();
    let edited = cart_service(true);
    let (out, _, code) = run_with(
        &refs(&base),
        &[(SERVICE, &edited)],
        &["--fail-on", "warning"],
    );
    assert_eq!(test_lines(&out).len(), 1, "{out}");
    assert_eq!(code, Some(1));
}

// "The spec changed too"
#[test]
fn the_spec_changed_too() {
    let base = angular();
    let (edited, spec2) = (cart_service(true), spec(2));
    let (out, _, _) = run(&refs(&base), &[(SERVICE, &edited), (SPEC, &spec2)]);
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
}

// "Test files regardless of exclusion": a spec renamed in the change counts
// as changed.
#[test]
fn a_renamed_spec_counts_as_changed() {
    let base = angular();
    let edited = cart_service(true);
    let spec1 = spec(1);
    let (out, _, _) = run(
        &refs(&base),
        &[
            (SERVICE, &edited),
            (SPEC, ""),
            ("src/app/cart.service.test.ts", &spec1),
        ],
    );
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
}

// "Excluded specs still count"
#[test]
fn excluded_specs_still_count() {
    let mut base = angular();
    base.push((
        "qgate.toml",
        "[files]\nexclude = [\"**/*.spec.ts\"]\n".to_owned(),
    ));
    let (edited, spec2) = (cart_service(true), spec(2));
    let (out, _, _) = run(&refs(&base), &[(SERVICE, &edited), (SPEC, &spec2)]);
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
    // And an excluded spec is still the service's test.
    let (out, _, _) = run(&refs(&base), &[(SERVICE, &edited)]);
    assert_eq!(test_lines(&out).len(), 1, "{out}");
}

// "Test files regardless of exclusion": a generated test counts by its
// path, as an excluded one does.
#[test]
fn a_generated_test_still_counts() {
    let mut base = solution();
    let header = "// <auto-generated />\n";
    let test = |value| format!("{header}{}", cs_class("OrderTests", value));
    let (before, after) = (test(1), test(2));
    for (path, contents) in &mut base {
        if *path == "tests/Domain.UnitTests/OrderTests.cs" {
            contents.clone_from(&before);
        }
    }
    let edited = cs_class("Order", 2);
    let (out, _, _) = run(
        &refs(&base),
        &[
            ("src/Domain/Order.cs", &edited),
            ("tests/Domain.UnitTests/OrderTests.cs", &after),
        ],
    );
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
    let (out, _, _) = run(&refs(&base), &[("src/Domain/Order.cs", &edited)]);
    assert_eq!(test_lines(&out).len(), 1, "{out}");
}

// "Changed units": a field is no code of a unit.
#[test]
fn a_field_edit_changes_no_unit() {
    let with_field = |limit: u32| {
        cart_service(false).replace(
            "export class CartService {\n",
            &format!("export class CartService {{\n  private limit = {limit};\n\n"),
        )
    };
    let mut base = angular();
    base[1].1 = with_field(1);
    let edited = with_field(2);
    let (out, _, _) = run(&refs(&base), &[(SERVICE, &edited)]);
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
}

// "A new function in a tested file"
#[test]
fn a_new_function_in_a_tested_file() {
    let base = angular();
    let added = cart_service(false).replace(
        "  remove(): number {",
        "  clear(): number {\n    return 0;\n  }\n\n  remove(): number {",
    );
    let (out, _, _) = run(&refs(&base), &[(SERVICE, &added)]);
    assert_eq!(
        test_lines(&out),
        [
            "warning src/app/cart.service.ts:12  CartService.clear changed, but not its test src/app/cart.service.spec.ts  [tests/unchanged-test]"
        ],
        "{out}"
    );
}

// "Changed units": "Only imports and types", "Removed function"
#[test]
fn edits_that_change_no_unit() {
    let base = angular();
    let imports = format!(
        "import {{ Other }} from \"./other\";\n{}\nexport interface Summary {{ count: number; }}\n",
        cart_service(false)
    );
    let removed = cart_service(false).replace("  add(): number {\n    return 2;\n  }\n\n", "");
    for change in [imports, removed] {
        let (out, _, _) = run(&refs(&base), &[(SERVICE, &change)]);
        assert_eq!(test_lines(&out), Vec::<&str>::new(), "{change}\n{out}");
    }
}

// "Renamed without edits"
#[test]
fn renamed_without_edits() {
    let base = angular();
    let same = cart_service(false);
    let (out, _, _) = run(
        &refs(&base),
        &[(SERVICE, ""), ("src/app/basket.service.ts", &same)],
    );
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
}

// "Checked changes": "Unparsable source file"
#[test]
fn unparsable_source_file() {
    let base = angular();
    let broken = cart_service(true).replace("total(): number {", "total(): number {{");
    let (out, _, _) = run(&refs(&base), &[(SERVICE, &broken)]);
    assert!(out.contains("[input/unparsable]"), "{out}");
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
}

// "Checked changes": "Working tree"
#[test]
fn working_tree() {
    let repo = Repo::new();
    for (path, contents) in angular() {
        repo.write(path, contents);
    }
    repo.commit("base");
    repo.write(SERVICE, cart_service(true));
    let out = stdout(&repo.qgate(&["--base", "HEAD", "--worktree"]));
    assert_eq!(test_lines(&out).len(), 1, "{out}");
    assert!(out.contains("src/app/cart.service.ts:4"), "{out}");
}

// "Checked changes": "Two-file form"
#[test]
fn two_file_form() {
    let repo = Repo::new();
    repo.write("package.json", "{}\n");
    repo.write("x.spec.ts", spec(1));
    repo.write("before.ts", "");
    repo.write("after.ts", function("f", 1));
    let out = qgate_in(&repo.dir, &["before.ts", "after.ts"]);
    assert_eq!(test_lines(&stdout(&out)), Vec::<&str>::new());
}

// "TypeScript tests": "A changed spec alone"
#[test]
fn a_changed_spec_alone() {
    let base = angular();
    let spec2 = spec(2);
    let (out, _, _) = run(&refs(&base), &[(SPEC, &spec2)]);
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
}

// "Missing test finding": "New Angular service"
#[test]
fn new_angular_service() {
    let base = [
        ("package.json", "{}\n"),
        (
            "src/app/cart/cart.component.spec.ts",
            "describe(\"c\", () => {});\n",
        ),
    ];
    let service = "import { Order } from \"./order\";\n\n\n\n\n\nexport class OrderService {\n  load() {\n    return 1;\n  }\n\n  save() {\n    return 2;\n  }\n}\n";
    let (out, _, _) = run(&base, &[("src/app/orders/order.service.ts", service)]);
    assert_eq!(
        test_lines(&out),
        [
            "warning src/app/orders/order.service.ts:8  OrderService.load and 1 more have no test: expected src/app/orders/order.service.spec.ts  [tests/missing-test]"
        ],
        "{out}"
    );
    // "A new file with its spec"
    let (out, _, _) = run(
        &base,
        &[
            ("src/app/orders/order.service.ts", service),
            (
                "src/app/orders/order.service.spec.ts",
                "describe(\"o\", () => {});\n",
            ),
        ],
    );
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
    // "A new file with only types"
    let (out, _, _) = run(
        &base,
        &[(
            "src/app/orders/order.model.ts",
            "export interface Order { id: string; }\n",
        )],
    );
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
}

// "An existing file without a test"
#[test]
fn an_existing_file_without_a_test() {
    let base = [
        ("package.json", "{}\n"),
        (
            "src/app/cart/cart.component.spec.ts",
            "describe(\"c\", () => {});\n",
        ),
        ("src/app/legacy/report.ts", &function("report", 1)),
    ];
    let edited = function("report", 2);
    let (out, _, _) = run(&base, &[("src/app/legacy/report.ts", &edited)]);
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
}

// "Projects that test other code": "Repository without specs", "Monorepo"
#[test]
fn projects_that_test_other_code() {
    let new = function("f", 1);
    let (out, _, _) = run(&[("package.json", "{}\n")], &[("src/format.ts", &new)]);
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");

    let base = [
        ("apps/web/package.json", "{}\n"),
        ("apps/web/src/a.spec.ts", "describe(\"a\", () => {});\n"),
        ("libs/util/package.json", "{}\n"),
    ];
    let (out, _, _) = run(
        &base,
        &[
            ("apps/web/src/new.ts", &new),
            ("libs/util/src/new.ts", &new),
        ],
    );
    let lines = test_lines(&out);
    assert_eq!(lines.len(), 1, "{out}");
    assert!(
        lines[0].starts_with("warning apps/web/src/new.ts:1  "),
        "{out}"
    );
}

/// A Clean Architecture-style solution: `Domain` with `Order.cs` and its
/// test in `Domain.UnitTests`, `Application` with two test projects, and
/// `Web` without one.
fn solution() -> Vec<(&'static str, String)> {
    vec![
        ("src/Domain/Domain.csproj", CSPROJ.to_owned()),
        ("src/Domain/Order.cs", cs_class("Order", 1)),
        (
            "tests/Domain.UnitTests/Domain.UnitTests.csproj",
            CSPROJ.to_owned(),
        ),
        (
            "tests/Domain.UnitTests/OrderTests.cs",
            cs_class("OrderTests", 1),
        ),
        (
            "tests/Domain.IntegrationTests/Domain.IntegrationTests.csproj",
            CSPROJ.to_owned(),
        ),
        ("src/Application/Application.csproj", CSPROJ.to_owned()),
        ("src/Application/TodoItems.cs", cs_class("TodoItems", 1)),
        (
            "tests/Application.UnitTests/Application.UnitTests.csproj",
            CSPROJ.to_owned(),
        ),
        (
            "tests/Application.UnitTests/TodoItemsTests.cs",
            cs_class("TodoItemsTests", 1),
        ),
        (
            "tests/Application.FunctionalTests/Application.FunctionalTests.csproj",
            CSPROJ.to_owned(),
        ),
        (
            "tests/Application.FunctionalTests/TodoItemsTests.cs",
            cs_class("TodoItemsFunctionalTests", 1),
        ),
        ("src/Web/Web.csproj", CSPROJ.to_owned()),
        ("src/Web/Program.cs", cs_class("Program", 1)),
    ]
}

// "Unchanged test finding": "One unit"
#[test]
fn one_csharp_unit() {
    let base = solution();
    let edited = cs_class("Order", 2);
    let (out, _, _) = run(&refs(&base), &[("src/Domain/Order.cs", &edited)]);
    assert_eq!(
        test_lines(&out),
        [
            "warning src/Domain/Order.cs:5  Order.Ship() changed, but not its test tests/Domain.UnitTests/OrderTests.cs  [tests/unchanged-test]"
        ],
        "{out}"
    );
}

// "One of several tests changed"
#[test]
fn one_of_several_csharp_tests_changed() {
    let base = solution();
    let (edited, test) = (
        cs_class("TodoItems", 2),
        cs_class("TodoItemsFunctionalTests", 2),
    );
    let (out, _, _) = run(
        &refs(&base),
        &[
            ("src/Application/TodoItems.cs", &edited),
            ("tests/Application.FunctionalTests/TodoItemsTests.cs", &test),
        ],
    );
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
}

// "Missing test finding": "New C# class"; "C# project without a test
// project"; "A file outside any project"
#[test]
fn new_csharp_classes() {
    let base = solution();
    let (invoice, orders, build) = (
        cs_class("Invoice", 1),
        cs_class("Orders", 1),
        cs_class("Build", 1),
    );
    let (out, _, _) = run(
        &refs(&base),
        &[
            ("src/Domain/Invoice.cs", &invoice),
            ("src/Web/Endpoints/Orders.cs", &orders),
            ("scripts/Build.cs", &build),
        ],
    );
    assert_eq!(
        test_lines(&out),
        [
            "warning src/Domain/Invoice.cs:5  Invoice.Ship() has no test: expected InvoiceTests.cs in Domain.UnitTests  [tests/missing-test]"
        ],
        "{out}"
    );
}

// "C# projects": "A changed test project alone"
#[test]
fn a_changed_test_project_alone() {
    let base = solution();
    let test = cs_class("OrderTests", 2);
    let (out, _, _) = run(
        &refs(&base),
        &[("tests/Domain.UnitTests/OrderTests.cs", &test)],
    );
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
}

// "Sources that need no test": "Environment files"
#[test]
fn environment_files() {
    let mut base = angular();
    let env = function("config", 1);
    // Without the setting, a new file without a spec.
    let (out, _, _) = run(
        &refs(&base),
        &[("src/environments/environment.prod.ts", &env)],
    );
    assert_eq!(test_lines(&out).len(), 1, "{out}");
    base.push((
        "qgate.toml",
        "[tests]\nexclude = [\"src/environments/**\"]\n".to_owned(),
    ));
    let (out, _, _) = run(
        &refs(&base),
        &[("src/environments/environment.prod.ts", &env)],
    );
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
}

// "Sources that need no test": "Other analyzers still run"
#[test]
fn other_analyzers_still_run() {
    let mut base = solution();
    base.push(("tests/Web.Tests/Web.Tests.csproj", CSPROJ.to_owned()));
    base.push((
        "tests/Web.Tests/ProgramTests.cs",
        cs_class("ProgramTests", 1),
    ));
    let mut ifs = String::new();
    for i in 0..14 {
        let _ = writeln!(ifs, "        if (d > {i}) {{ d--; }}");
    }
    let program = format!(
        "namespace Shop;\n\npublic class Program\n{{\n    public int Ship()\n    {{\n        return 1;\n    }}\n\n    public int Run(int d)\n    {{\n{ifs}        return d;\n    }}\n}}\n"
    );
    // Without the setting, `ProgramTests.cs` did not change.
    let (out, _, _) = run(&refs(&base), &[("src/Web/Program.cs", &program)]);
    assert_eq!(test_lines(&out).len(), 1, "{out}");
    base.push((
        "qgate.toml",
        "[tests]\nexclude = [\"**/Program.cs\"]\n".to_owned(),
    ));
    let (out, _, _) = run(&refs(&base), &[("src/Web/Program.cs", &program)]);
    assert!(
        out.contains("src/Web/Program.cs:10  Program.Run(int): new function, cyclomatic complexity 15  [complexity/cyclomatic-new]"),
        "{out}"
    );
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
}

// configuration: "The tests table"
#[test]
fn the_tests_table() {
    let base = angular();
    let edited = cart_service(true);
    let (out, err, code) = run(
        &refs(&base),
        &[
            ("qgate.toml", "[tests]\nexclude = [\"/src/**\"]\n"),
            (SERVICE, &edited),
        ],
    );
    assert_eq!(out, "");
    assert!(err.contains("qgate.toml"), "{err}");
    assert!(err.contains("tests.exclude"), "{err}");
    assert!(err.contains("\"/src/**\""), "{err}");
    assert_eq!(code, Some(2));

    let (out, err, code) = run(
        &refs(&base),
        &[
            (
                "qgate.toml",
                "[tests]\nexlude = [\"src/environments/**\"]\n",
            ),
            (SERVICE, &edited),
        ],
    );
    assert_eq!(out, "");
    assert!(
        err.contains("qgate.toml") && err.contains("exlude"),
        "{err}"
    );
    assert_eq!(code, Some(2));

    let (out, _, _) = run(
        &refs(&base),
        &[
            ("qgate.toml", "[tests]\nexclude = [\"**\"]\n"),
            (SERVICE, &edited),
        ],
    );
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
}

// "TypeScript test code": "Test helpers", "A directory whose name only
// contains test"
#[test]
fn test_helpers_by_directory() {
    let base = angular();
    let helper = function("helper", 1);
    let (out, _, _) = run(
        &refs(&base),
        &[
            ("src/app/spec/test-utils.ts", &helper),
            ("e2e/cart-helpers.ts", &helper),
        ],
    );
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
    let (out, _, _) = run(&refs(&base), &[("src/testing/format.ts", &helper)]);
    assert_eq!(test_lines(&out).len(), 1, "{out}");
    assert!(out.contains("[tests/missing-test]"), "{out}");
}

// "C# tests": "Names that differ in case"
#[test]
fn project_names_that_differ_in_case() {
    let base = [
        ("src/Sso/Sso.csproj", CSPROJ.to_owned()),
        (
            "src/Sso/AccountController.cs",
            cs_class("AccountController", 1),
        ),
        ("test/SSO.Test/SSO.Test.csproj", CSPROJ.to_owned()),
        (
            "test/SSO.Test/AccountControllerTest.cs",
            cs_class("AccountControllerTest", 1),
        ),
    ];
    let (edited, test) = (
        cs_class("AccountController", 2),
        cs_class("AccountControllerTest", 2),
    );
    let (out, _, _) = run(
        &refs(&base),
        &[
            ("src/Sso/AccountController.cs", &edited),
            ("test/SSO.Test/AccountControllerTest.cs", &test),
        ],
    );
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
    let (out, _, _) = run(&refs(&base), &[("src/Sso/AccountController.cs", &edited)]);
    assert_eq!(
        test_lines(&out),
        [
            "warning src/Sso/AccountController.cs:5  AccountController.Ship() changed, but not its test test/SSO.Test/AccountControllerTest.cs  [tests/unchanged-test]"
        ],
        "{out}"
    );
}

// "Feature test projects", "Only an acceptance test project"
#[test]
fn feature_test_projects() {
    let mut base = solution();
    base.push((
        "tests/Web.AcceptanceTests/Web.AcceptanceTests.csproj",
        CSPROJ.to_owned(),
    ));
    let (handler, orders, todo) = (
        cs_class("Handler", 1),
        cs_class("Orders", 1),
        cs_class("TodoItems", 2),
    );
    // Only an acceptance test project: no missing test expected.
    let (out, _, _) = run(
        &refs(&base),
        &[("src/Web/Infrastructure/Handler.cs", &handler)],
    );
    assert_eq!(test_lines(&out), Vec::<&str>::new(), "{out}");
    // The expected project is a class test project.
    let (out, _, _) = run(&refs(&base), &[("src/Application/Orders.cs", &orders)]);
    assert!(
        out.contains("expected OrdersTests.cs in Application.UnitTests  [tests/missing-test]"),
        "{out}"
    );
    // Functional tests still count: the first unchanged test by path is
    // the functional one.
    let (out, _, _) = run(&refs(&base), &[("src/Application/TodoItems.cs", &todo)]);
    assert!(
        out.contains("but not its test tests/Application.FunctionalTests/TodoItemsTests.cs, and 1 more  [tests/unchanged-test]"),
        "{out}"
    );
}
