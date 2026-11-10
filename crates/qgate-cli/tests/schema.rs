//! Tests for the schema test helper in `support/schema.rs`, and for
//! `docs/schema/report-v1.json` itself: the helper must reject what the
//! schema forbids, or the end-to-end checks against it prove nothing.

mod support;

use serde_json::{Value, json};
use support::schema::{report_v1, unsupported, validate};

/// A report with every key the schema knows.
fn full_report() -> Value {
    json!({
        "schema_version": 1,
        "findings": [
            {
                "rule": "complexity/cyclomatic-increase",
                "severity": "warning",
                "message": "f: cyclomatic complexity 2 -> 4 (+2)",
                "path": "src/a.ts",
                "line": 1,
                "metric": {"before": 2, "after": 4}
            },
            {
                "rule": "duplication/duplicate-block",
                "severity": "error",
                "message": "duplicates src/b.ts:1-6 (lines 4-9, 68 tokens)",
                "path": "src/a.ts",
                "line": 4,
                "end_line": 9,
                "related": [{"path": "src/b.ts", "line": 1, "end_line": 6}]
            },
            {
                "rule": "input/unparsable",
                "severity": "info",
                "message": "file not analyzed: not valid UTF-8",
                "path": "src/c.ts"
            }
        ],
        "summary": {"count": 3, "failing": 1, "fail_on": "error"}
    })
}

fn assert_valid(report: &Value) {
    if let Err(errors) = validate(&report_v1(), report) {
        panic!("{report} is invalid: {errors:?}");
    }
}

/// Change `report` with `edit` and expect the schema to reject it.
fn assert_rejected(edit: impl FnOnce(&mut Value)) {
    let mut report = full_report();
    edit(&mut report);
    assert!(
        validate(&report_v1(), &report).is_err(),
        "{report} was accepted"
    );
}

#[test]
fn the_schema_uses_only_supported_keywords() {
    assert_eq!(unsupported(&report_v1()), Vec::<String>::new());
}

#[test]
fn the_guard_finds_an_unsupported_keyword() {
    let mut schema = report_v1();
    schema["$defs"]["line"]["maximum"] = json!(10);
    schema["properties"]["findings"]["items"]["pattern"] = json!("x");
    schema["$defs"]["metric"]["anyOf"][1]["minProperties"] = json!(1);
    assert_eq!(
        unsupported(&schema),
        [
            "$.$defs.line.maximum",
            "$.$defs.metric.anyOf[1].minProperties",
            "$.properties.findings.items.pattern",
        ]
    );
}

#[test]
fn the_guard_finds_a_value_shape_the_helper_does_not_read() {
    for (pointer, value) in [
        ("/$defs/line/type", json!(["integer"])),
        ("/$defs/line/type", json!("int")),
        ("/$defs/line/minimum", json!("1")),
        ("/$defs/location/required", json!([1])),
        ("/$defs/finding/properties/severity/enum", json!("error")),
        ("/$defs/finding/properties/related/minItems", json!(-1)),
        ("/$defs/finding/properties/line/$ref", json!("#/$defs/nope")),
        (
            "/properties/summary/$ref",
            json!({"$ref": "#/$defs/summary"}),
        ),
        ("/$defs/metric/anyOf", json!([])),
    ] {
        let mut schema = report_v1();
        *schema.pointer_mut(pointer).expect(pointer) = value.clone();
        let found = unsupported(&schema);
        assert_eq!(found.len(), 1, "{pointer} = {value}: {found:?}");
        assert!(found[0].contains("unsupported value"), "{found:?}");
    }
}

#[test]
fn a_valid_report_is_accepted() {
    assert_valid(&full_report());
    assert_valid(&json!({
        "schema_version": 1,
        "findings": [],
        "summary": {"count": 0, "failing": 0, "fail_on": "never"}
    }));
}

#[test]
fn a_metric_with_one_value_is_accepted() {
    for metric in [json!({"before": 3}), json!({"after": 12})] {
        let mut report = full_report();
        report["findings"][0]["metric"] = metric;
        assert_valid(&report);
    }
}

#[test]
fn fractional_metric_values_are_accepted() {
    let mut report = full_report();
    report["findings"][0]["metric"] = json!({"before": 0.5, "after": 1.25});
    assert_valid(&report);
}

#[test]
fn unknown_keys_are_accepted() {
    let mut report = full_report();
    report["generator"] = json!("qgate 9.9");
    report["findings"][0]["details"] = json!({"target": "../db"});
    assert_valid(&report);
}

// report-formats: Versioned JSON format, "A rule id the schema does not list"
#[test]
fn a_rule_id_the_schema_does_not_list() {
    let mut report = full_report();
    report["findings"][0]["rule"] = json!("security/hardcoded-secret");
    assert_valid(&report);
}

#[test]
fn a_missing_key_is_rejected() {
    assert_rejected(|r| {
        r.as_object_mut().expect("object").remove("summary");
    });
    assert_rejected(|r| {
        r["findings"][0]
            .as_object_mut()
            .expect("object")
            .remove("path");
    });
    assert_rejected(|r| {
        r["findings"][1]["related"][0]
            .as_object_mut()
            .expect("object")
            .remove("line");
    });
    assert_rejected(|r| {
        r["summary"]
            .as_object_mut()
            .expect("object")
            .remove("fail_on");
    });
}

#[test]
fn a_wrong_type_is_rejected() {
    assert_rejected(|r| r["findings"][0]["line"] = json!("1"));
    assert_rejected(|r| r["findings"][0]["line"] = json!(1.0));
    assert_rejected(|r| r["findings"][0]["metric"]["after"] = json!("4"));
    assert_rejected(|r| r["findings"][1]["related"] = json!({"path": "src/b.ts", "line": 1}));
    assert_rejected(|r| r["summary"]["count"] = json!(-1));
    assert_rejected(|r| r["findings"] = json!({}));
}

#[test]
fn a_null_is_rejected() {
    assert_rejected(|r| r["findings"][0]["line"] = Value::Null);
    assert_rejected(|r| r["findings"][0]["metric"]["before"] = Value::Null);
    assert_rejected(|r| r["findings"][0]["metric"] = Value::Null);
    assert_rejected(|r| r["findings"][0]["severity"] = Value::Null);
    assert_rejected(|r| r["findings"][1]["end_line"] = Value::Null);
}

#[test]
fn what_version_1_rules_out_is_rejected() {
    assert_rejected(|r| r["schema_version"] = json!(2));
    assert_rejected(|r| r["findings"][0]["severity"] = json!("critical"));
    assert_rejected(|r| r["findings"][0]["line"] = json!(0));
    assert_rejected(|r| r["findings"][0]["metric"] = json!({}));
    assert_rejected(|r| r["findings"][0]["metric"] = json!({"unit": "x"}));
    assert_rejected(|r| r["findings"][1]["related"] = json!([]));
}
