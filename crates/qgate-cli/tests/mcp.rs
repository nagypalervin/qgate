//! End-to-end tests for `qgate mcp`, one per scenario of the `mcp-server`
//! spec. Each test builds a throwaway git repository, starts the server,
//! writes the client's messages to its standard input, closes it, and
//! reads the response lines (design D9 of `add-mcp-server`).

mod support;

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};
use support::schema::{report_v1, validate};
use support::{Repo, function, isolate, qgate_in, scratch_dir, stderr, stdout};

/// Start `qgate mcp <args>` in `dir`, send `messages`, one per line, close
/// standard input and wait for the server to end.
fn serve(dir: &Path, args: &[&str], messages: &[String]) -> Output {
    let mut child = isolate(&mut Command::new(env!("CARGO_BIN_EXE_qgate")))
        .arg("mcp")
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("qgate runs");
    let mut input = String::new();
    for message in messages {
        input.push_str(message);
        input.push('\n');
    }
    // A server that rejects its command line exits before reading.
    let _ = child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes());
    child.wait_with_output().expect("qgate finishes")
}

/// The response lines of a server that ended with 0, parsed.
fn responses(out: &Output) -> Vec<Value> {
    assert_eq!(out.status.code(), Some(0), "{}", stderr(out));
    stdout(out)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line:?}")))
        .collect()
}

fn request(id: i64, method: &str, params: &Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

/// `_meta` declaring the current revision.
fn current() -> Value {
    json!({"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}})
}

fn call_message(arguments: &Value) -> String {
    let mut params = current();
    params["name"] = json!("check");
    params["arguments"] = arguments.clone();
    request(1, "tools/call", &params)
}

/// The tool result of one `check` call with `arguments`, by a server
/// started in `dir` with `args`.
fn call_in(dir: &Path, args: &[&str], arguments: &Value) -> Value {
    let responses = responses(&serve(dir, args, &[call_message(arguments)]));
    assert_eq!(responses.len(), 1, "{responses:?}");
    assert_eq!(responses[0]["id"], 1);
    responses[0]["result"].clone()
}

fn call(repo: &Repo, arguments: &Value) -> Value {
    call_in(&repo.dir, &[], arguments)
}

/// The report of a check that ran.
fn report(result: &Value) -> &Value {
    assert_eq!(result["isError"], false, "{result}");
    &result["structuredContent"]
}

/// The `path` and the function named in each complexity finding's
/// message. `f` and `g` are copies of each other, so a duplication
/// finding may come with them; the scenarios are about the complexity.
fn found(result: &Value) -> Vec<String> {
    report(result)["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|f| f["rule"] == "complexity/cyclomatic-increase")
        .map(|f| {
            let message = f["message"].as_str().expect("message");
            let name = message.split(':').next().unwrap_or_default();
            format!("{} {name}", f["path"].as_str().expect("path"))
        })
        .collect()
}

/// The text of a tool error.
fn error_text(result: &Value) -> String {
    assert_eq!(result["isError"], true, "{result}");
    assert!(result.get("structuredContent").is_none(), "{result}");
    let content = result["content"].as_array().expect("content");
    assert_eq!(content.len(), 1, "{result}");
    let text = content[0]["text"].as_str().expect("text").to_owned();
    assert!(text.starts_with("qgate: "), "{text}");
    text
}

/// A repository with `files` committed on `main`.
fn committed(files: &[(&str, &str)]) -> Repo {
    let repo = Repo::new();
    for (path, contents) in files {
        repo.write(path, contents);
    }
    repo.commit("base");
    repo
}

/// `f` in `src/a.ts` at complexity 2, committed; on disk at `after`.
fn f_goes_to(after: usize) -> Repo {
    let repo = committed(&[("src/a.ts", &function("f", 1))]);
    repo.write("src/a.ts", function("f", after - 1));
    repo
}

/// `main` has `f` and `g` at complexity 2; the `feature` branch commits
/// `f` at 9, and `g` is at 9 on disk only.
fn committed_and_uncommitted() -> Repo {
    let repo = committed(&[
        ("src/a.ts", &function("f", 1)),
        ("src/b.ts", &function("g", 1)),
    ]);
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("src/a.ts", function("f", 8));
    repo.commit("f gets complex");
    repo.write("src/b.ts", function("g", 8));
    repo
}

// Requirement: Server command

#[test]
fn input_ends() {
    let repo = f_goes_to(9);
    let out = serve(&repo.dir, &[], &[]);
    assert_eq!(stdout(&out), "");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

#[test]
fn help_lists_the_mcp_command() {
    let out = Command::new(env!("CARGO_BIN_EXE_qgate"))
        .arg("--help")
        .output()
        .expect("qgate runs");
    assert!(stdout(&out).contains("mcp"), "{}", stdout(&out));
}

#[test]
fn unknown_fail_on_level() {
    let repo = f_goes_to(9);
    let out = serve(
        &repo.dir,
        &["--fail-on", "info"],
        &[call_message(&json!({}))],
    );
    assert!(
        stderr(&out).contains("[possible values: error, warning, never]"),
        "{}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "");
    assert_eq!(out.status.code(), Some(2));
}

// Requirement: Current protocol revision

#[test]
fn discovery() {
    let repo = f_goes_to(9);
    let out = serve(
        &repo.dir,
        &[],
        &[r#"{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}"#.to_owned()],
    );
    let responses = responses(&out);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["id"], 1);
    let result = &responses[0]["result"];
    assert_eq!(result["resultType"], "complete");
    assert!(
        result["supportedVersions"]
            .as_array()
            .is_some_and(|v| v.contains(&json!("2026-07-28"))),
        "{result}"
    );
    assert!(result["capabilities"].get("tools").is_some(), "{result}");
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"],
        json!({"name": "qgate", "version": env!("CARGO_PKG_VERSION")})
    );
}

#[test]
fn a_call_without_a_handshake() {
    let repo = f_goes_to(9);
    let responses = responses(&serve(
        &repo.dir,
        &[],
        &[request(1, "tools/list", &current())],
    ));
    assert_eq!(responses[0]["result"]["tools"][0]["name"], "check");
}

// Requirement: Unsupported protocol revision

#[test]
fn a_future_revision() {
    let repo = f_goes_to(9);
    let mut params = current();
    params["_meta"]["io.modelcontextprotocol/protocolVersion"] = json!("2099-01-01");
    let responses = responses(&serve(&repo.dir, &[], &[request(1, "tools/list", &params)]));
    let error = &responses[0]["error"];
    assert_eq!(error["code"], -32022);
    assert_eq!(error["data"]["requested"], "2099-01-01");
    assert!(
        error["data"]["supported"]
            .as_array()
            .is_some_and(|v| v.contains(&json!("2026-07-28"))),
        "{error}"
    );
}

// Requirement: Older protocol revisions

fn initialize(version: &str) -> String {
    request(
        0,
        "initialize",
        &json!({
            "protocolVersion": version,
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "0"}
        }),
    )
}

#[test]
fn handshake() {
    let repo = f_goes_to(9);
    let responses = responses(&serve(&repo.dir, &[], &[initialize("2025-11-25")]));
    let result = &responses[0]["result"];
    assert_eq!(result["protocolVersion"], "2025-11-25");
    assert!(result["capabilities"].get("tools").is_some(), "{result}");
    assert_eq!(result["serverInfo"]["name"], "qgate");
}

#[test]
fn an_older_revision_the_server_does_not_serve() {
    let repo = f_goes_to(9);
    let responses = responses(&serve(&repo.dir, &[], &[initialize("2024-11-05")]));
    assert_eq!(responses[0]["result"]["protocolVersion"], "2025-11-25");
}

#[test]
fn ping() {
    let repo = f_goes_to(9);
    let out = serve(
        &repo.dir,
        &[],
        &[
            initialize("2025-11-25"),
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_owned(),
            r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#.to_owned(),
        ],
    );
    let text = stdout(&out);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert_eq!(lines[1], r#"{"jsonrpc":"2.0","id":7,"result":{}}"#);
}

#[test]
fn a_whole_old_era_session() {
    let repo = f_goes_to(9);
    let out = serve(
        &repo.dir,
        &[],
        &[
            initialize("2025-11-25"),
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_owned(),
            request(1, "tools/list", &json!({})),
            request(2, "tools/call", &json!({"name": "check", "arguments": {}})),
        ],
    );
    let responses = responses(&out);
    assert_eq!(responses.len(), 3);
    let call = &responses[2]["result"];
    assert_eq!(found(call), ["src/a.ts f"]);
    assert!(call.get("resultType").is_none(), "{call}");
}

// Requirement: The check tool

#[test]
fn listing_the_tools() {
    let repo = f_goes_to(9);
    let responses = responses(&serve(
        &repo.dir,
        &[],
        &[request(1, "tools/list", &current())],
    ));
    let tools = responses[0]["result"]["tools"].as_array().expect("tools");
    assert_eq!(tools.len(), 1);
    let tool = &tools[0];
    assert_eq!(tool["name"], "check");
    let properties = tool["inputSchema"]["properties"]
        .as_object()
        .expect("properties");
    let mut names: Vec<&str> = properties.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, ["base", "directory", "head"]);
    assert!(tool["inputSchema"].get("required").is_none(), "{tool}");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["outputSchema"], report_v1());
}

// Requirement: What the tool checks

#[test]
fn uncommitted_work() {
    let repo = committed_and_uncommitted();
    assert_eq!(found(&call(&repo, &json!({}))), ["src/b.ts g"]);
}

#[test]
fn the_whole_branch() {
    let repo = committed_and_uncommitted();
    assert_eq!(
        found(&call(&repo, &json!({"base": "main"}))),
        ["src/a.ts f", "src/b.ts g"]
    );
}

#[test]
fn a_revision_instead_of_the_working_tree() {
    let repo = committed_and_uncommitted();
    assert_eq!(
        found(&call(&repo, &json!({"base": "main", "head": "HEAD"}))),
        ["src/a.ts f"]
    );
}

// Requirement: The tool's result

#[test]
fn one_failing_finding() {
    let repo = f_goes_to(9);
    let result = call(&repo, &json!({}));
    let report = report(&result);
    assert_eq!(report["schema_version"], 1);
    let findings = report["findings"].as_array().expect("findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["rule"], "complexity/cyclomatic-increase");
    assert_eq!(findings[0]["severity"], "error");
    assert_eq!(findings[0]["path"], "src/a.ts");
    assert_eq!(
        report["summary"],
        json!({"count": 1, "failing": 1, "fail_on": "error"})
    );
    let content = result["content"].as_array().expect("content");
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");
    let text: Value =
        serde_json::from_str(content[0]["text"].as_str().expect("text")).expect("JSON text");
    assert_eq!(&text, report);
}

#[test]
fn the_report_is_the_check_commands_report() {
    let repo = committed_and_uncommitted();
    let result = call(&repo, &json!({"base": "main"}));
    let report = report(&result);
    validate(&report_v1(), report).unwrap_or_else(|errors| panic!("{errors:?}"));
    let out = qgate_in(
        &repo.dir,
        &["--base", "main", "--worktree", "--format", "json"],
    );
    let check = stdout(&out);
    assert_eq!(
        result["content"][0]["text"].as_str(),
        Some(check.trim_end_matches('\n'))
    );
    assert_eq!(
        &serde_json::from_str::<Value>(&check).expect("JSON"),
        report
    );
}

#[test]
fn nothing_got_worse() {
    let repo = committed(&[("src/a.ts", &function("f", 1))]);
    let result = call(&repo, &json!({}));
    assert_eq!(report(&result)["findings"], json!([]));
}

#[test]
fn the_teams_level() {
    let repo = f_goes_to(4);
    let result = call_in(&repo.dir, &["--fail-on", "warning"], &json!({}));
    assert_eq!(
        report(&result)["summary"],
        json!({"count": 1, "failing": 1, "fail_on": "warning"})
    );
}

// Requirement: The directory checked

#[test]
fn default_directory() {
    let repo = f_goes_to(9);
    assert_eq!(found(&call(&repo, &json!({}))), ["src/a.ts f"]);
}

#[test]
fn another_directory() {
    let repo = f_goes_to(9);
    let outside = scratch_dir("no-repo");
    let dir = repo.dir.to_str().expect("UTF-8 path");
    let result = call_in(&outside, &[], &json!({"directory": dir}));
    assert_eq!(found(&result), ["src/a.ts f"]);
    let _ = std::fs::remove_dir_all(outside);
}

#[test]
fn a_relative_directory() {
    let repo = f_goes_to(9);
    let parent = repo.dir.parent().expect("parent");
    let name = repo.dir.file_name().and_then(|n| n.to_str()).expect("name");
    let result = call_in(parent, &[], &json!({"directory": name}));
    assert_eq!(found(&result), ["src/a.ts f"]);
}

// Requirement: A check that cannot run

#[test]
fn unknown_base() {
    let repo = f_goes_to(9);
    let out = serve(
        &repo.dir,
        &[],
        &[
            call_message(&json!({"base": "no-such-branch"})),
            request(2, "tools/list", &current()),
        ],
    );
    let responses = responses(&out);
    let text = error_text(&responses[0]["result"]);
    assert!(text.contains("no-such-branch"), "{text}");
    assert_eq!(responses[1]["result"]["tools"][0]["name"], "check");
}

#[test]
fn invalid_configuration() {
    let repo = f_goes_to(9);
    repo.write("qgate.toml", "[architecture\n");
    let text = error_text(&call(&repo, &json!({})));
    assert!(text.contains("qgate.toml"), "{text}");
}

#[test]
fn outside_a_repository() {
    let outside = scratch_dir("no-repo");
    error_text(&call_in(&outside, &[], &json!({})));
    let _ = std::fs::remove_dir_all(outside);
}

// Requirement: Invalid arguments

#[test]
fn a_value_that_is_not_a_string() {
    let repo = f_goes_to(9);
    let text = error_text(&call(&repo, &json!({"base": 7})));
    assert!(text.contains("\"base\""), "{text}");
}

#[test]
fn unknown_tool() {
    let repo = f_goes_to(9);
    let mut params = current();
    params["name"] = json!("explain");
    let responses = responses(&serve(&repo.dir, &[], &[request(1, "tools/call", &params)]));
    assert_eq!(responses[0]["error"]["code"], -32602);
}

// Requirement: Protocol errors

#[test]
fn not_json() {
    let repo = f_goes_to(9);
    let out = serve(
        &repo.dir,
        &[],
        &["not json".to_owned(), request(2, "tools/list", &current())],
    );
    let responses = responses(&out);
    assert_eq!(responses[0]["error"]["code"], -32700);
    assert_eq!(responses[0]["id"], Value::Null);
    assert_eq!(responses[1]["result"]["tools"][0]["name"], "check");
}

#[test]
fn unknown_method() {
    let repo = f_goes_to(9);
    let out = serve(
        &repo.dir,
        &[],
        &[r#"{"jsonrpc":"2.0","id":3,"method":"resources/list"}"#.to_owned()],
    );
    let responses = responses(&out);
    assert_eq!(responses[0]["id"], 3);
    assert_eq!(responses[0]["error"]["code"], -32601);
}

#[test]
fn a_cancellation() {
    let repo = f_goes_to(9);
    let out = serve(
        &repo.dir,
        &[],
        &[
            r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#
                .to_owned(),
        ],
    );
    assert_eq!(stdout(&out), "");
    assert_eq!(out.status.code(), Some(0));
}

// Requirement: The server changes nothing

#[test]
fn staged_work_stays_staged() {
    let repo = committed(&[
        ("src/a.ts", &function("f", 1)),
        ("src/b.ts", &function("g", 1)),
    ]);
    repo.write("src/a.ts", function("f", 8));
    repo.git(&["add", "src/a.ts"]);
    repo.write("src/b.ts", function("g", 8));
    let before = repo.git(&["status", "--porcelain"]);
    assert_eq!(
        found(&call(&repo, &json!({}))),
        ["src/a.ts f", "src/b.ts g"]
    );
    assert_eq!(repo.git(&["status", "--porcelain"]), before);
}
