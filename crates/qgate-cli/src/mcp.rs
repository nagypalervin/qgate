//! `qgate mcp`: a Model Context Protocol server on standard input and
//! output, with one tool, `check` (the `mcp-server` spec, design of
//! `add-mcp-server`).
//!
//! Everything here is pure: `main` reads the lines, passes the check as a
//! callback and writes the responses (design D1).

use std::path::{Path, PathBuf};

use qgate_core::Finding;
use qgate_report::{ReportSummary, render_json};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::{FailOn, HeadArg};

/// The current protocol revision: no handshake, the version in every
/// request's `_meta` (design D3).
const CURRENT: &str = "2026-07-28";

/// The revisions with a handshake that the server answers as requested;
/// the first is the one it offers for any other.
const OLD: [&str; 2] = ["2025-11-25", "2025-06-18"];

/// Where a request's `_meta` declares its protocol revision.
const VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";

/// The one tool.
const TOOL: &str = "check";

/// The JSON report's schema, version 1: the tool's output schema (design
/// D4). The published file is the only copy.
const REPORT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/schema/report-v1.json"
));

/// What the model reads about the server (design D7).
const INSTRUCTIONS: &str = "qgate reports what a change made worse. Call check before you hand \
     work back or commit it. Fix the findings that summary.failing counts, or tell the user \
     why such a change is intended; fix the others when it is cheap.";

/// What the model reads about the tool (design D7).
const DESCRIPTION: &str = "Report what the changes made worse: higher complexity, duplicated \
     code, broken architecture rules. Without arguments it checks the uncommitted work (the \
     working tree, untracked files included) against HEAD. Pass base \"main\" (or the branch \
     this work will merge into) to check the whole branch; pass head to check a commit \
     instead of the working tree. Each finding is a delta this change made worse; \
     summary.failing counts the findings the team treats as failing.";

/// JSON-RPC error codes the server uses.
const PARSE_ERROR: i64 = -32_700;
const INVALID_REQUEST: i64 = -32_600;
const METHOD_NOT_FOUND: i64 = -32_601;
const INVALID_PARAMS: i64 = -32_602;
const UNSUPPORTED_VERSION: i64 = -32_022;

/// The protocol era of the process: current until a client sends
/// `initialize` (design D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Era {
    Current,
    Old,
}

/// A JSON-RPC error.
#[derive(Debug, Serialize)]
struct RpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl RpcError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

/// A response, with its keys in the order JSON-RPC examples use.
#[derive(Serialize)]
struct Response<'a> {
    jsonrpc: &'static str,
    id: &'a Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

/// The arguments of a `check` call.
#[derive(Debug, Default)]
struct Arguments {
    base: Option<String>,
    head: Option<String>,
    directory: Option<String>,
}

/// The server: the level the report's summary counts, the directory it
/// was started in, the check, and the protocol era the client chose.
pub struct Server<C> {
    fail_on: FailOn,
    cwd: Result<PathBuf, String>,
    check: C,
    era: Era,
}

impl<C> Server<C>
where
    C: FnMut(&Path, &str, HeadArg<'_>) -> Result<Vec<Finding>, String>,
{
    /// A server whose summary counts findings at `fail_on`, that checks
    /// `cwd` by default (or reports why it cannot), and runs checks with
    /// `check`.
    pub fn new(fail_on: FailOn, cwd: Result<PathBuf, String>, check: C) -> Self {
        Self {
            fail_on,
            cwd,
            check,
            era: Era::Current,
        }
    }

    /// The response to one input line, without a newline, or `None` when
    /// the line gets no response: an empty line or a notification
    /// (design D8).
    pub fn handle(&mut self, line: &str) -> Option<String> {
        if line.trim().is_empty() {
            return None;
        }
        let message: Value = match serde_json::from_str(line) {
            Ok(message) => message,
            Err(e) => {
                let error = RpcError::new(PARSE_ERROR, format!("the message is not JSON: {e}"));
                return Some(respond(&Value::Null, Err(error)));
            }
        };
        let Some(object) = message.as_object() else {
            let why = if message.is_array() {
                "batches are not supported; send one message per line"
            } else {
                "a message must be a JSON object"
            };
            return Some(respond(
                &Value::Null,
                Err(RpcError::new(INVALID_REQUEST, why)),
            ));
        };
        let id = object.get("id");
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            let error = RpcError::new(INVALID_REQUEST, "a request needs a method");
            return Some(respond(id.unwrap_or(&Value::Null), Err(error)));
        };
        // A message without an id is a notification: never answered.
        let id = id?;
        let params = object.get("params");
        Some(respond(id, self.dispatch(method, params)))
    }

    /// The result of the request `method` with `params`.
    fn dispatch(&mut self, method: &str, params: Option<&Value>) -> Result<Value, RpcError> {
        if method == "initialize" {
            self.era = Era::Old;
            return Ok(handshake(params));
        }
        let era = match params
            .and_then(|p| p.get("_meta"))
            .and_then(|m| m.get(VERSION_KEY))
        {
            Some(Value::String(version)) if version == CURRENT => Era::Current,
            Some(requested) => {
                return Err(RpcError {
                    data: Some(json!({"supported": [CURRENT], "requested": requested})),
                    ..RpcError::new(
                        UNSUPPORTED_VERSION,
                        format!("unsupported protocol version; this server speaks {CURRENT}"),
                    )
                });
            }
            None => self.era,
        };
        let mut result = match method {
            "server/discover" => discovery(),
            "ping" => json!({}),
            "tools/list" => {
                let mut list = json!({"tools": [tool()]});
                if era == Era::Current {
                    list["ttlMs"] = json!(0);
                    list["cacheScope"] = json!("public");
                }
                list
            }
            "tools/call" => self.call(params)?,
            _ => {
                return Err(RpcError::new(
                    METHOD_NOT_FOUND,
                    format!("unknown method \"{method}\""),
                ));
            }
        };
        if era == Era::Current {
            result["resultType"] = json!("complete");
        }
        Ok(result)
    }

    /// The result of `tools/call`: a tool result, or a protocol error for
    /// a call of a tool other than `check` (design D4, D5).
    fn call(&mut self, params: Option<&Value>) -> Result<Value, RpcError> {
        let name = params
            .and_then(|p| p.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| RpcError::new(INVALID_PARAMS, "tools/call needs the name of a tool"))?;
        if name != TOOL {
            return Err(RpcError::new(
                INVALID_PARAMS,
                format!("unknown tool \"{name}\"; the only tool is {TOOL}"),
            ));
        }
        Ok(match self.run(params.and_then(|p| p.get("arguments"))) {
            Ok(report) => {
                let structured: Value =
                    serde_json::from_str(&report).expect("the JSON report is JSON");
                json!({
                    "content": [{"type": "text", "text": report}],
                    "structuredContent": structured,
                    "isError": false,
                })
            }
            Err(message) => json!({
                "content": [{"type": "text", "text": format!("qgate: {message}")}],
                "isError": true,
            }),
        })
    }

    /// Run the check the arguments ask for, and return the JSON report
    /// without its newline, or the message why it could not run.
    fn run(&mut self, arguments: Option<&Value>) -> Result<String, String> {
        let arguments = parse_arguments(arguments)?;
        let dir = match arguments.directory {
            Some(dir) if Path::new(&dir).is_absolute() => PathBuf::from(dir),
            Some(dir) => self.cwd.clone()?.join(dir),
            None => self.cwd.clone()?,
        };
        let base = arguments.base.as_deref().unwrap_or("HEAD");
        let head = arguments
            .head
            .as_deref()
            .map_or(HeadArg::WorkTree, HeadArg::Revision);
        let findings = (self.check)(&dir, base, head)?;
        let report = render_json(
            &findings,
            ReportSummary {
                failing: self.fail_on.failing(&findings),
                fail_on: self.fail_on.name(),
            },
        );
        Ok(report.trim_end_matches('\n').to_owned())
    }
}

/// One response line.
fn respond(id: &Value, outcome: Result<Value, RpcError>) -> String {
    let (result, error) = match outcome {
        Ok(result) => (Some(result), None),
        Err(error) => (None, Some(error)),
    };
    let response = Response {
        jsonrpc: "2.0",
        id,
        result,
        error,
    };
    serde_json::to_string(&response).expect("a response always serializes")
}

/// The arguments of a `check` call, or a message that names what is
/// wrong with them (design D5).
fn parse_arguments(arguments: Option<&Value>) -> Result<Arguments, String> {
    let object = match arguments {
        None | Some(Value::Null) => return Ok(Arguments::default()),
        Some(Value::Object(object)) => object,
        Some(_) => return Err("the arguments must be an object".to_owned()),
    };
    let mut parsed = Arguments::default();
    for (key, value) in object {
        let slot = match key.as_str() {
            "base" => &mut parsed.base,
            "head" => &mut parsed.head,
            "directory" => &mut parsed.directory,
            _ => {
                return Err(format!(
                    "unknown argument \"{key}\"; the arguments are base, head and directory"
                ));
            }
        };
        let Value::String(text) = value else {
            return Err(format!("argument \"{key}\" must be a string"));
        };
        *slot = Some(text.clone());
    }
    Ok(parsed)
}

/// What the server is, for discovery and the handshake alike.
fn server_info() -> Value {
    json!({"name": "qgate", "version": env!("CARGO_PKG_VERSION")})
}

fn capabilities() -> Value {
    json!({"tools": {"listChanged": false}})
}

/// The answer to `server/discover` (current era).
fn discovery() -> Value {
    json!({
        "supportedVersions": [CURRENT],
        "capabilities": capabilities(),
        "instructions": INSTRUCTIONS,
        "_meta": {"io.modelcontextprotocol/serverInfo": server_info()},
    })
}

/// The answer to `initialize` (old era): the requested revision when the
/// server serves it, else the latest old one.
fn handshake(params: Option<&Value>) -> Value {
    let requested = params
        .and_then(|p| p.get("protocolVersion"))
        .and_then(Value::as_str);
    let version = requested.filter(|v| OLD.contains(v)).unwrap_or(OLD[0]);
    json!({
        "protocolVersion": version,
        "capabilities": capabilities(),
        "serverInfo": server_info(),
        "instructions": INSTRUCTIONS,
    })
}

/// The `check` tool as `tools/list` shows it.
fn tool() -> Value {
    let schema: Value =
        serde_json::from_str(REPORT_SCHEMA).expect("the embedded report schema is JSON");
    let string = |description: &str| json!({"type": "string", "description": description});
    let mut properties = Map::new();
    properties.insert(
        "base".into(),
        string(
            "The revision to compare against: HEAD (the default) for the uncommitted work, \
             or the branch this work will merge into, such as main, for the whole branch.",
        ),
    );
    properties.insert(
        "head".into(),
        string(
            "A revision to check instead of the working tree, such as HEAD for the last \
             commit.",
        ),
    );
    properties.insert(
        "directory".into(),
        string(
            "The repository to check, absolute or relative to where the server started. \
             Default: where the server started.",
        ),
    );
    json!({
        "name": TOOL,
        "title": "Check what got worse",
        "description": DESCRIPTION,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "additionalProperties": false,
        },
        "outputSchema": schema,
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use qgate_core::{Location, Severity};
    use serde_json::{Value, json};

    use super::*;

    /// A check call as the callback saw it: directory, base, and the head
    /// revision (`None` for the working tree).
    type Call = (PathBuf, String, Option<String>);

    /// The fake check of the tests.
    type FakeCheck = Box<dyn FnMut(&Path, &str, HeadArg<'_>) -> Result<Vec<Finding>, String>>;

    fn finding(severity: Severity, message: &str) -> Finding {
        Finding {
            rule: "complexity/cyclomatic-increase".into(),
            severity,
            message: message.into(),
            location: Location {
                path: "src/a.ts".into(),
                line: Some(1),
                end_line: None,
            },
            metric: None,
            related: Vec::new(),
        }
    }

    fn error_f() -> Finding {
        finding(Severity::Error, "f: cyclomatic complexity 2 -> 9 (+7)")
    }

    fn warning_f() -> Finding {
        finding(Severity::Warning, "f: cyclomatic complexity 2 -> 4 (+2)")
    }

    const CWD: &str = "/work/app";

    /// A server started in `CWD` at `fail_on`, whose check returns
    /// `result` and records its calls.
    fn server_with(
        fail_on: FailOn,
        result: Result<Vec<Finding>, String>,
    ) -> (Server<FakeCheck>, Rc<RefCell<Vec<Call>>>) {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&calls);
        let check: FakeCheck = Box::new(move |dir: &Path, base: &str, head: HeadArg<'_>| {
            let head = match head {
                HeadArg::Revision(rev) => Some(rev.to_owned()),
                HeadArg::WorkTree => None,
            };
            seen.borrow_mut()
                .push((dir.to_path_buf(), base.to_owned(), head));
            result.clone()
        });
        (Server::new(fail_on, Ok(PathBuf::from(CWD)), check), calls)
    }

    fn server() -> Server<FakeCheck> {
        server_with(FailOn::Error, Ok(vec![error_f()])).0
    }

    /// The response to `message`, parsed; it must be one line.
    fn ask<C>(server: &mut Server<C>, message: &Value) -> Value
    where
        C: FnMut(&Path, &str, HeadArg<'_>) -> Result<Vec<Finding>, String>,
    {
        let line = server
            .handle(&message.to_string())
            .expect("a response to a request");
        assert!(!line.contains('\n'), "one line: {line}");
        serde_json::from_str(&line).expect("the response is JSON")
    }

    fn request(id: i64, method: &str, params: &Value) -> Value {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
    }

    /// `_meta` declaring the current revision.
    fn current() -> Value {
        json!({"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}})
    }

    fn call(arguments: &Value) -> Value {
        let mut params = current();
        params["name"] = json!("check");
        params["arguments"] = arguments.clone();
        request(5, "tools/call", &params)
    }

    fn result(response: &Value) -> &Value {
        assert_eq!(response["jsonrpc"], "2.0", "{response}");
        response
            .get("result")
            .unwrap_or_else(|| panic!("a result: {response}"))
    }

    fn error_code(response: &Value) -> i64 {
        assert_eq!(response["jsonrpc"], "2.0", "{response}");
        response["error"]["code"]
            .as_i64()
            .unwrap_or_else(|| panic!("an error: {response}"))
    }

    fn initialize(version: Option<&str>) -> Value {
        let mut params =
            json!({"capabilities": {}, "clientInfo": {"name": "test", "version": "0"}});
        if let Some(version) = version {
            params["protocolVersion"] = json!(version);
        }
        request(0, "initialize", &params)
    }

    fn tool_text(result: &Value) -> &str {
        let content = result["content"].as_array().expect("content");
        assert_eq!(content.len(), 1, "one block: {result}");
        assert_eq!(content[0]["type"], "text");
        content[0]["text"].as_str().expect("text")
    }

    fn report_schema() -> Value {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/schema/report-v1.json"
        );
        serde_json::from_str(&std::fs::read_to_string(path).expect("read the schema"))
            .expect("the schema is JSON")
    }

    // Discovery and handshake.

    #[test]
    fn discovery_names_the_revision_tools_and_server() {
        let response = ask(&mut server(), &request(1, "server/discover", &current()));
        assert_eq!(response["id"], 1);
        let result = result(&response);
        assert_eq!(result["resultType"], "complete");
        assert_eq!(result["supportedVersions"], json!(["2026-07-28"]));
        assert_eq!(
            result["capabilities"],
            json!({"tools": {"listChanged": false}})
        );
        assert_eq!(
            result["_meta"]["io.modelcontextprotocol/serverInfo"],
            json!({"name": "qgate", "version": env!("CARGO_PKG_VERSION")})
        );
        assert!(
            result["instructions"]
                .as_str()
                .is_some_and(|text| text.contains("check")),
            "{result}"
        );
    }

    #[test]
    fn handshake_answers_the_versions_it_serves() {
        for version in ["2025-11-25", "2025-06-18"] {
            let response = ask(&mut server(), &initialize(Some(version)));
            assert_eq!(result(&response)["protocolVersion"], version);
        }
    }

    #[test]
    fn handshake_offers_its_latest_old_version_otherwise() {
        for version in [Some("2024-11-05"), Some("2026-07-28"), None] {
            let response = ask(&mut server(), &initialize(version));
            assert_eq!(
                result(&response)["protocolVersion"],
                "2025-11-25",
                "{version:?}"
            );
        }
    }

    #[test]
    fn handshake_says_what_discovery_says() {
        let mut server = server();
        let discovery = ask(&mut server, &request(1, "server/discover", &current()));
        let handshake = ask(&mut server, &initialize(Some("2025-11-25")));
        let (discovery, handshake) = (result(&discovery), result(&handshake));
        assert_eq!(handshake["capabilities"], discovery["capabilities"]);
        assert_eq!(handshake["instructions"], discovery["instructions"]);
        assert_eq!(
            handshake["serverInfo"],
            discovery["_meta"]["io.modelcontextprotocol/serverInfo"]
        );
        assert!(handshake.get("resultType").is_none(), "{handshake}");
    }

    // Eras.

    #[test]
    fn a_request_without_a_handshake_is_served() {
        let response = ask(&mut server(), &request(2, "tools/list", &json!({})));
        let result = result(&response);
        assert_eq!(result["tools"][0]["name"], "check");
        assert_eq!(result["resultType"], "complete");
    }

    #[test]
    fn the_current_era_marks_results() {
        let response = ask(&mut server(), &request(2, "tools/list", &current()));
        let result = result(&response);
        assert_eq!(result["resultType"], "complete");
        assert_eq!(result["ttlMs"], 0);
        assert_eq!(result["cacheScope"], "public");
        let response = ask(&mut server(), &call(&json!({})));
        assert_eq!(super::tests::result(&response)["resultType"], "complete");
    }

    #[test]
    fn after_a_handshake_results_are_not_marked() {
        let mut server = server();
        ask(&mut server, &initialize(Some("2025-11-25")));
        assert_eq!(
            server.handle(
                &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string()
            ),
            None
        );
        let list = ask(&mut server, &request(1, "tools/list", &json!({})));
        let mut params = json!({"name": "check"});
        params["arguments"] = json!({});
        let call = ask(&mut server, &request(2, "tools/call", &params));
        for response in [&list, &call] {
            let result = result(response);
            for key in ["resultType", "ttlMs", "cacheScope"] {
                assert!(result.get(key).is_none(), "{key} in {result}");
            }
        }
    }

    #[test]
    fn an_unknown_declared_revision_is_rejected() {
        let mut params = current();
        params["_meta"]["io.modelcontextprotocol/protocolVersion"] = json!("2099-01-01");
        let response = ask(&mut server(), &request(4, "tools/list", &params));
        assert_eq!(response["id"], 4);
        assert_eq!(error_code(&response), -32022);
        assert_eq!(response["error"]["data"]["requested"], "2099-01-01");
        assert_eq!(
            response["error"]["data"]["supported"],
            json!(["2026-07-28"])
        );
    }

    #[test]
    fn ping_gets_an_empty_result() {
        let mut server = server();
        ask(&mut server, &initialize(Some("2025-11-25")));
        let line = server.handle(r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#);
        assert_eq!(
            line.as_deref(),
            Some(r#"{"jsonrpc":"2.0","id":7,"result":{}}"#)
        );
    }

    #[test]
    fn notifications_get_no_response() {
        let mut server = server();
        for method in [
            "notifications/initialized",
            "notifications/cancelled",
            "notifications/something-new",
        ] {
            let line = json!({"jsonrpc": "2.0", "method": method, "params": {"requestId": 1}});
            assert_eq!(server.handle(&line.to_string()), None, "{method}");
        }
    }

    // The tool list.

    #[test]
    fn one_tool_with_three_optional_string_arguments() {
        let response = ask(&mut server(), &request(2, "tools/list", &current()));
        let tools = result(&response)["tools"].as_array().expect("tools");
        assert_eq!(tools.len(), 1);
        let tool = &tools[0];
        assert_eq!(tool["name"], "check");
        assert!(
            tool["title"].is_string() && tool["description"].is_string(),
            "{tool}"
        );
        let input = &tool["inputSchema"];
        assert_eq!(input["type"], "object");
        assert_eq!(input["additionalProperties"], false);
        let properties = input["properties"].as_object().expect("properties");
        let names: Vec<&str> = properties.keys().map(String::as_str).collect();
        assert_eq!(names, ["base", "directory", "head"]);
        for property in properties.values() {
            assert_eq!(property["type"], "string");
        }
        assert!(input.get("required").is_none(), "{input}");
        assert_eq!(
            tool["annotations"],
            json!({
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            })
        );
    }

    #[test]
    fn output_schema_is_the_report_schema() {
        let response = ask(&mut server(), &request(2, "tools/list", &current()));
        assert_eq!(
            result(&response)["tools"][0]["outputSchema"],
            report_schema()
        );
    }

    // Calls.

    #[test]
    fn no_arguments_check_the_working_tree_against_head_where_started() {
        for arguments in [None, Some(json!(null)), Some(json!({}))] {
            let (mut server, calls) = server_with(FailOn::Error, Ok(Vec::new()));
            let mut params = current();
            params["name"] = json!("check");
            if let Some(arguments) = arguments {
                params["arguments"] = arguments;
            }
            ask(&mut server, &request(5, "tools/call", &params));
            assert_eq!(
                *calls.borrow(),
                [(PathBuf::from(CWD), "HEAD".to_owned(), None)]
            );
        }
    }

    #[test]
    fn arguments_reach_the_check() {
        let cases = [
            (json!({"base": "main"}), (CWD, "main", None)),
            (
                json!({"base": "main", "head": "HEAD"}),
                (CWD, "main", Some("HEAD")),
            ),
            (
                json!({"directory": "/other/repo"}),
                ("/other/repo", "HEAD", None),
            ),
            (
                json!({"directory": "sub/dir"}),
                ("/work/app/sub/dir", "HEAD", None),
            ),
        ];
        for (arguments, (dir, base, head)) in cases {
            let (mut server, calls) = server_with(FailOn::Error, Ok(Vec::new()));
            ask(&mut server, &call(&arguments));
            assert_eq!(
                *calls.borrow(),
                [(PathBuf::from(dir), base.to_owned(), head.map(str::to_owned))],
                "{arguments}"
            );
        }
    }

    #[test]
    fn findings_become_the_report_twice() {
        let (mut server, _) = server_with(FailOn::Error, Ok(vec![error_f(), warning_f()]));
        let response = ask(&mut server, &call(&json!({})));
        let result = result(&response);
        assert_eq!(result["isError"], false);
        let report = &result["structuredContent"];
        assert_eq!(report["schema_version"], 1);
        assert_eq!(report["findings"].as_array().map(Vec::len), Some(2));
        assert_eq!(
            report["summary"],
            json!({"count": 2, "failing": 1, "fail_on": "error"})
        );
        let text = tool_text(result);
        let expected = qgate_report::render_json(
            &[error_f(), warning_f()],
            qgate_report::ReportSummary {
                failing: 1,
                fail_on: "error",
            },
        );
        assert_eq!(text, expected.trim_end_matches('\n'));
        assert_eq!(
            serde_json::from_str::<Value>(text).expect("JSON text"),
            *report
        );
    }

    #[test]
    fn the_summary_counts_at_the_servers_level() {
        let (mut server, _) = server_with(FailOn::Warning, Ok(vec![warning_f()]));
        let response = ask(&mut server, &call(&json!({})));
        assert_eq!(
            result(&response)["structuredContent"]["summary"],
            json!({"count": 1, "failing": 1, "fail_on": "warning"})
        );
    }

    #[test]
    fn a_check_that_cannot_run_is_a_tool_error() {
        let (mut server, _) = server_with(
            FailOn::Error,
            Err("unknown revision \"no-such-branch\"".to_owned()),
        );
        let response = ask(&mut server, &call(&json!({"base": "no-such-branch"})));
        let result = result(&response);
        assert_eq!(result["isError"], true);
        assert!(result.get("structuredContent").is_none(), "{result}");
        assert_eq!(
            tool_text(result),
            "qgate: unknown revision \"no-such-branch\""
        );
        // The server goes on.
        let response = ask(&mut server, &request(6, "tools/list", &current()));
        assert_eq!(super::tests::result(&response)["tools"][0]["name"], "check");
    }

    #[test]
    fn an_unreadable_start_directory_is_a_tool_error_when_needed() {
        let calls = Rc::new(RefCell::new(0));
        let seen = Rc::clone(&calls);
        let mut server = Server::new(
            FailOn::Error,
            Err("cannot read the current directory: gone".to_owned()),
            move |_: &Path, _: &str, _: HeadArg<'_>| {
                *seen.borrow_mut() += 1;
                Ok(Vec::new())
            },
        );
        let response = ask(&mut server, &call(&json!({})));
        let result = result(&response);
        assert_eq!(result["isError"], true);
        assert_eq!(
            tool_text(result),
            "qgate: cannot read the current directory: gone"
        );
        // An absolute directory does not need it.
        let response = ask(&mut server, &call(&json!({"directory": "/work/app"})));
        assert_eq!(super::tests::result(&response)["isError"], false);
        assert_eq!(*calls.borrow(), 1);
    }

    #[test]
    fn invalid_arguments_are_tool_errors_that_name_them() {
        let cases = [
            (
                json!({"foo": "x"}),
                "qgate: unknown argument \"foo\"; the arguments are base, head and directory",
            ),
            (
                json!({"base": 7}),
                "qgate: argument \"base\" must be a string",
            ),
            (
                json!({"head": null}),
                "qgate: argument \"head\" must be a string",
            ),
            (json!(["main"]), "qgate: the arguments must be an object"),
        ];
        for (arguments, text) in cases {
            let (mut server, calls) = server_with(FailOn::Error, Ok(Vec::new()));
            let response = ask(&mut server, &call(&arguments));
            let result = result(&response);
            assert_eq!(result["isError"], true, "{arguments}");
            assert_eq!(tool_text(result), text);
            assert!(calls.borrow().is_empty(), "no check for {arguments}");
        }
    }

    #[test]
    fn an_unknown_or_missing_tool_is_a_protocol_error() {
        let mut params = current();
        params["name"] = json!("explain");
        assert_eq!(
            error_code(&ask(&mut server(), &request(5, "tools/call", &params))),
            -32602
        );
        assert_eq!(
            error_code(&ask(&mut server(), &request(5, "tools/call", &current()))),
            -32602
        );
    }

    // Protocol errors and the envelope.

    #[test]
    fn a_line_that_is_not_json() {
        let mut server = server();
        let response = ask_line(&mut server, "not json");
        assert_eq!(error_code(&response), -32700);
        assert_eq!(response["id"], Value::Null);
        assert!(response.as_object().is_some_and(|o| o.contains_key("id")));
        let response = ask(&mut server, &request(6, "tools/list", &current()));
        assert_eq!(result(&response)["tools"][0]["name"], "check");
    }

    fn ask_line<C>(server: &mut Server<C>, line: &str) -> Value
    where
        C: FnMut(&Path, &str, HeadArg<'_>) -> Result<Vec<Finding>, String>,
    {
        let line = server.handle(line).expect("a response");
        serde_json::from_str(&line).expect("the response is JSON")
    }

    #[test]
    fn invalid_requests() {
        let mut server = server();
        let batch = ask_line(&mut server, r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#);
        assert_eq!(error_code(&batch), -32600);
        assert_eq!(batch["id"], Value::Null);
        let no_method = ask_line(&mut server, r#"{"jsonrpc":"2.0","id":3}"#);
        assert_eq!(error_code(&no_method), -32600);
        assert_eq!(no_method["id"], 3);
        let not_an_object = ask_line(&mut server, "42");
        assert_eq!(error_code(&not_an_object), -32600);
    }

    #[test]
    fn an_unknown_method() {
        let response = ask(&mut server(), &request(3, "resources/list", &json!({})));
        assert_eq!(response["id"], 3);
        assert_eq!(error_code(&response), -32601);
    }

    #[test]
    fn empty_lines_get_no_response() {
        assert_eq!(server().handle(""), None);
        assert_eq!(server().handle("  \t"), None);
    }

    #[test]
    fn string_ids_come_back() {
        let response = ask_line(
            &mut server(),
            r#"{"jsonrpc":"2.0","id":"probe-1","method":"server/discover","params":{}}"#,
        );
        assert_eq!(response["id"], "probe-1");
        assert!(response.get("result").is_some(), "{response}");
    }
}
