//! A JSON Schema validator for `docs/schema/report-v1.json`, just large
//! enough for that schema (design D8 of `add-report-formats`). It knows the
//! keywords in [`ASSERTIONS`]; `keywords_outside` finds any other one, so a
//! test fails before this validator skips a rule silently.
//!
//! Stricter than JSON Schema in one way: `integer` means a JSON number
//! written without a fraction or exponent, because qgate promises that.

use serde_json::Value;

/// The keywords this validator checks.
pub const ASSERTIONS: &[&str] = &[
    "type",
    "properties",
    "required",
    "items",
    "enum",
    "const",
    "minimum",
    "minItems",
    "minProperties",
    "$ref",
];

/// Keywords that only describe or hold definitions; nothing to check.
pub const ANNOTATIONS: &[&str] = &["$schema", "$id", "title", "description", "$defs"];

/// The version 1 schema, read from the repository.
pub fn report_v1() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/schema/report-v1.json"
    );
    let text = std::fs::read_to_string(path).expect("read docs/schema/report-v1.json");
    serde_json::from_str(&text).expect("the schema is JSON")
}

/// Check `instance` against `schema`, and return every violation, each
/// with the JSON path where it is.
pub fn validate(schema: &Value, instance: &Value) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    check(schema, schema, instance, "$", &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check(root: &Value, schema: &Value, instance: &Value, at: &str, errors: &mut Vec<String>) {
    let Some(schema) = schema.as_object() else {
        errors.push(format!("{at}: schema is not an object"));
        return;
    };
    if let Some(target) = schema.get("$ref").and_then(Value::as_str) {
        match target
            .strip_prefix("#/$defs/")
            .and_then(|name| root.get("$defs")?.get(name))
        {
            Some(def) => check(root, def, instance, at, errors),
            None => errors.push(format!("{at}: unknown $ref {target}")),
        }
    }
    let mut fail = |what: String| errors.push(format!("{at}: {what}"));
    if let Some(expected) = schema.get("type").and_then(Value::as_str) {
        let ok = match expected {
            "object" => instance.is_object(),
            "array" => instance.is_array(),
            "string" => instance.is_string(),
            "number" => instance.is_number(),
            "integer" => instance.is_i64() || instance.is_u64(),
            "boolean" => instance.is_boolean(),
            "null" => instance.is_null(),
            other => {
                fail(format!("unknown type {other}"));
                true
            }
        };
        if !ok {
            fail(format!("expected {expected}, got {instance}"));
        }
    }
    if let Some(value) = schema.get("const") {
        if instance != value {
            fail(format!("expected {value}, got {instance}"));
        }
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        if !values.contains(instance) {
            fail(format!("{instance} is not one of {values:?}"));
        }
    }
    if let (Some(min), Some(n)) = (
        schema.get("minimum").and_then(Value::as_f64),
        instance.as_f64(),
    ) {
        if n < min {
            fail(format!("{n} is below the minimum {min}"));
        }
    }
    if let (Some(min), Some(items)) = (
        schema.get("minItems").and_then(Value::as_u64),
        instance.as_array(),
    ) {
        if (items.len() as u64) < min {
            fail(format!("fewer than {min} items"));
        }
    }
    let object = instance.as_object();
    if let (Some(min), Some(object)) = (schema.get("minProperties").and_then(Value::as_u64), object)
    {
        if (object.len() as u64) < min {
            fail(format!("fewer than {min} properties"));
        }
    }
    if let (Some(required), Some(object)) =
        (schema.get("required").and_then(Value::as_array), object)
    {
        for key in required.iter().filter_map(Value::as_str) {
            if !object.contains_key(key) {
                fail(format!("missing {key:?}"));
            }
        }
    }
    if let (Some(properties), Some(object)) =
        (schema.get("properties").and_then(Value::as_object), object)
    {
        for (key, sub) in properties {
            if let Some(value) = object.get(key) {
                check(root, sub, value, &format!("{at}.{key}"), errors);
            }
        }
    }
    if let (Some(sub), Some(items)) = (schema.get("items"), instance.as_array()) {
        for (i, item) in items.iter().enumerate() {
            check(root, sub, item, &format!("{at}[{i}]"), errors);
        }
    }
}

/// Every keyword in `schema` that is neither in [`ASSERTIONS`] nor in
/// [`ANNOTATIONS`], with where it is.
pub fn keywords_outside(schema: &Value) -> Vec<String> {
    let mut found = Vec::new();
    walk(schema, "$", &mut found);
    found
}

fn walk(schema: &Value, at: &str, found: &mut Vec<String>) {
    let Some(schema) = schema.as_object() else {
        found.push(format!("{at}: not an object"));
        return;
    };
    for (key, value) in schema {
        if !ASSERTIONS.contains(&key.as_str()) && !ANNOTATIONS.contains(&key.as_str()) {
            found.push(format!("{at}.{key}"));
        }
        match key.as_str() {
            // Maps of subschemas.
            "properties" | "$defs" => {
                for (name, sub) in value.as_object().into_iter().flatten() {
                    walk(sub, &format!("{at}.{key}.{name}"), found);
                }
            }
            "items" => walk(value, &format!("{at}.items"), found),
            // Values, not schemas.
            _ => {}
        }
    }
}
