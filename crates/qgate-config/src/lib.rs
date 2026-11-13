//! Reads `qgate.toml`, the qgate configuration file, into plain data.
//!
//! The crate knows the file format and nothing else (ADR 0003): no I/O, no
//! analysis, no printing. Callers read the bytes (revision mode takes them
//! from the head revision) and hand the values to the analyzers, which
//! define their own rule types and check pattern syntax.
//!
//! ```
//! let config = qgate_config::parse(br#"
//! [[architecture.forbid]]
//! name = "domain-is-pure"
//! from = ["src/domain/**"]
//! to = ["src/ui/**", "react"]
//! "#)?;
//! assert_eq!(config.architecture.forbid[0].name, "domain-is-pure");
//! # Ok::<(), qgate_config::ConfigError>(())
//! ```

use std::collections::HashSet;
use std::fmt;

use serde::Deserialize;

/// The whole configuration. An empty file is a valid, empty configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// The `[architecture]` table.
    pub architecture: Architecture,
    /// The `[files]` table.
    pub files: Files,
}

/// The `[files]` table: which files revision mode leaves out (design D1
/// of `add-path-exclusion`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Files {
    /// Path patterns of files to leave out, as written. Their syntax is
    /// checked where they are interpreted; an empty list excludes nothing.
    pub exclude: Vec<String>,
}

/// The `[architecture]` table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Architecture {
    /// The `[[architecture.forbid]]` rules, in file order.
    pub forbid: Vec<ForbidRule>,
}

/// One `[[architecture.forbid]]` rule: files matching `from` must not
/// import targets matching `to`, nor use C# namespaces matching
/// `to_namespaces`. Patterns are kept as written; their syntax is checked
/// where they are interpreted. A rule has `to`, `to_namespaces` or both
/// (design D3 of `add-csharp-architecture`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForbidRule {
    /// Unique, non-empty name, shown in findings.
    pub name: String,
    /// Patterns for the paths of importing files. Never empty.
    pub from: Vec<String>,
    /// Patterns for import targets, if given. Never empty.
    #[serde(default)]
    pub to: Option<Vec<String>>,
    /// Patterns for the namespaces C# `using` directives name, if given.
    /// Never empty.
    #[serde(default)]
    pub to_namespaces: Option<Vec<String>>,
}

/// Why `qgate.toml` is not valid. The message names the problem, and the
/// line and column when the TOML parser knows them; it does not name the
/// file, which is the caller's to add.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}

impl ConfigError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Parse and check the bytes of `qgate.toml`.
///
/// # Errors
///
/// A [`ConfigError`] if the bytes are not UTF-8 or not TOML, contain an
/// unknown table or key or a value of the wrong type, or define a rule with
/// an empty or duplicate name, an empty `from`, `to` or `to_namespaces`
/// list, or neither `to` nor `to_namespaces`.
pub fn parse(bytes: &[u8]) -> Result<Config, ConfigError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ConfigError::new("not valid UTF-8"))?;
    let config: Config = toml::from_str(text).map_err(|e| toml_error(text, &e))?;
    check(&config)?;
    Ok(config)
}

/// The rules serde cannot express: names and non-empty lists.
fn check(config: &Config) -> Result<(), ConfigError> {
    let mut names = HashSet::new();
    for (index, rule) in config.architecture.forbid.iter().enumerate() {
        // A blank name could not tell findings apart, so it counts as empty.
        if rule.name.trim().is_empty() {
            return Err(ConfigError::new(format!(
                "architecture.forbid rule {}: `name` is empty",
                index + 1
            )));
        }
        if !names.insert(rule.name.as_str()) {
            return Err(ConfigError::new(format!(
                "architecture.forbid: duplicate rule name {:?}",
                rule.name
            )));
        }
        let lists = [
            ("from", Some(&rule.from)),
            ("to", rule.to.as_ref()),
            ("to_namespaces", rule.to_namespaces.as_ref()),
        ];
        for (key, list) in lists {
            if list.is_some_and(Vec::is_empty) {
                return Err(ConfigError::new(format!(
                    "architecture.forbid rule {:?}: `{key}` is empty",
                    rule.name
                )));
            }
        }
        if rule.to.is_none() && rule.to_namespaces.is_none() {
            return Err(ConfigError::new(format!(
                "architecture.forbid rule {:?}: needs `to` or `to_namespaces`",
                rule.name
            )));
        }
    }
    Ok(())
}

/// One line: `line L, column C: <message>`, or the message alone if the
/// parser gives no position.
fn toml_error(text: &str, error: &toml::de::Error) -> ConfigError {
    let message = error.message().trim_end();
    match error.span() {
        Some(span) => {
            let before = text.get(..span.start).unwrap_or(text);
            let line = before.matches('\n').count() + 1;
            // In characters, as editors count them, not bytes.
            let line_start = before.rfind('\n').map_or(0, |i| i + 1);
            let column = before[line_start..].chars().count() + 1;
            ConfigError::new(format!("line {line}, column {column}: {message}"))
        }
        None => ConfigError::new(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error(text: &str) -> String {
        parse(text.as_bytes())
            .expect_err("invalid configuration")
            .to_string()
    }

    const VALID: &str = r#"
[[architecture.forbid]]
name = "domain-is-pure"
from = ["src/domain/**"]
to = ["src/ui/**", "react"]

[[architecture.forbid]]
name = "no-db-in-ui"
from = ["src/ui/**"]
to = ["@prisma/client"]
"#;

    #[test]
    fn valid_file() {
        let config = parse(VALID.as_bytes()).expect("valid");
        assert_eq!(
            config.architecture.forbid,
            [
                ForbidRule {
                    name: "domain-is-pure".to_owned(),
                    from: vec!["src/domain/**".to_owned()],
                    to: Some(vec!["src/ui/**".to_owned(), "react".to_owned()]),
                    to_namespaces: None,
                },
                ForbidRule {
                    name: "no-db-in-ui".to_owned(),
                    from: vec!["src/ui/**".to_owned()],
                    to: Some(vec!["@prisma/client".to_owned()]),
                    to_namespaces: None,
                },
            ]
        );
    }

    #[test]
    fn empty_file_and_empty_table() {
        assert_eq!(parse(b"").expect("valid"), Config::default());
        assert_eq!(
            parse(b"[architecture]\n").expect("valid"),
            Config::default()
        );
    }

    #[test]
    fn syntax_error_names_its_line() {
        let text =
            "[[architecture.forbid]]\nfrom = [\"a\"]\nname = \"domain-is-pure\nto = [\"b\"]\n";
        let err = error(text);
        assert!(err.starts_with("line 3, column "), "{err}");
    }

    #[test]
    fn column_counts_characters_and_crlf_lines() {
        let text = "[architecture]\n# \u{e9}\u{6f22}\u{5b57}\nforbid = \"\u{e9}\u{6f22}\" x\n";
        // `x` is the 15th character on line 3 but its 18th byte.
        assert!(
            error(text).starts_with("line 3, column 15: "),
            "{}",
            error(text)
        );
        let crlf = "[[architecture.forbid]]\r\nfrom = [\"a\"]\r\nname = \"r\" x\r\n";
        assert!(
            error(crlf).starts_with("line 3, column 12: "),
            "{}",
            error(crlf)
        );
    }

    #[test]
    fn names_in_messages_are_escaped() {
        let rule = "[[architecture.forbid]]\nname = 'a\"b'\nfrom = [\"a\"]\nto = [\"b\"]\n";
        assert_eq!(
            error(&format!("{rule}{rule}")),
            "architecture.forbid: duplicate rule name \"a\\\"b\""
        );
    }

    #[test]
    fn misspelled_table() {
        let err = error("[[architecture.forbd]]\nname = \"x\"\n");
        assert!(err.contains("forbd"), "{err}");
        assert!(err.starts_with("line 1, "), "{err}");
    }

    #[test]
    fn unknown_key_in_a_rule_and_unknown_top_level_table() {
        let err = error(
            "[[architecture.forbid]]\nname = \"x\"\nfrom = [\"a\"]\nto = [\"b\"]\nseverity = \"warning\"\n",
        );
        assert!(err.contains("severity"), "{err}");
        let err = error("[complexity]\nmax = 10\n");
        assert!(err.contains("complexity"), "{err}");
    }

    #[test]
    fn wrong_type() {
        let err = error("[[architecture.forbid]]\nname = \"x\"\nfrom = \"src/**\"\nto = [\"b\"]\n");
        assert!(err.starts_with("line 3, "), "{err}");
    }

    // configuration: "Rule without targets"
    #[test]
    fn missing_key() {
        let err = error("[[architecture.forbid]]\nname = \"x\"\nfrom = [\"a\"]\n");
        assert_eq!(
            err,
            "architecture.forbid rule \"x\": needs `to` or `to_namespaces`"
        );
        let err = error("[[architecture.forbid]]\nname = \"x\"\nto = [\"a\"]\n");
        assert!(err.contains("from"), "{err}");
    }

    // configuration: "Only namespaces"; architecture-analysis: "Forbid rules"
    #[test]
    fn to_namespaces() {
        let rule = |lists: &str| {
            parse(
                format!("[[architecture.forbid]]\nname = \"r\"\nfrom = [\"a\"]\n{lists}")
                    .as_bytes(),
            )
            .expect("valid")
            .architecture
            .forbid
            .remove(0)
        };
        let only = rule("to_namespaces = [\"Shop.Infrastructure\"]\n");
        assert_eq!(only.to, None);
        assert_eq!(
            only.to_namespaces,
            Some(vec!["Shop.Infrastructure".to_owned()])
        );
        let both = rule("to = [\"src/ui/**\"]\nto_namespaces = [\"Shop.Web\"]\n");
        assert_eq!(both.to, Some(vec!["src/ui/**".to_owned()]));
        assert_eq!(both.to_namespaces, Some(vec!["Shop.Web".to_owned()]));
        let err = error(
            "[[architecture.forbid]]\nname = \"x\"\nfrom = [\"a\"]\nto_namespaces = \"Shop\"\n",
        );
        assert!(err.starts_with("line 4, "), "{err}");
    }

    #[test]
    fn empty_or_blank_name() {
        for name in ["", "  "] {
            let err = error(&format!(
                "[[architecture.forbid]]\nname = \"{name}\"\nfrom = [\"a\"]\nto = [\"b\"]\n"
            ));
            assert_eq!(err, "architecture.forbid rule 1: `name` is empty");
        }
    }

    #[test]
    fn duplicate_name() {
        let rule =
            "[[architecture.forbid]]\nname = \"domain-is-pure\"\nfrom = [\"a\"]\nto = [\"b\"]\n";
        let err = error(&format!("{rule}{rule}"));
        assert_eq!(
            err,
            "architecture.forbid: duplicate rule name \"domain-is-pure\""
        );
    }

    #[test]
    fn empty_lists() {
        let err = error("[[architecture.forbid]]\nname = \"r\"\nfrom = [\"a\"]\nto = []\n");
        assert_eq!(err, "architecture.forbid rule \"r\": `to` is empty");
        let err = error("[[architecture.forbid]]\nname = \"r\"\nfrom = []\nto = [\"b\"]\n");
        assert_eq!(err, "architecture.forbid rule \"r\": `from` is empty");
        // An empty list is an error even next to a full one.
        let err = error(
            "[[architecture.forbid]]\nname = \"r\"\nfrom = [\"a\"]\nto = [\"b\"]\nto_namespaces = []\n",
        );
        assert_eq!(
            err,
            "architecture.forbid rule \"r\": `to_namespaces` is empty"
        );
        let err = error(
            "[[architecture.forbid]]\nname = \"r\"\nfrom = [\"a\"]\nto = []\nto_namespaces = [\"S\"]\n",
        );
        assert_eq!(err, "architecture.forbid rule \"r\": `to` is empty");
    }

    // configuration: "Excluded paths"
    #[test]
    fn files_exclude() {
        let config =
            parse(b"[files]\nexclude = [\"**/Migrations/**\", \"vendor\"]\n").expect("valid");
        assert_eq!(config.files.exclude, ["**/Migrations/**", "vendor"]);
        assert_eq!(
            parse(b"[files]\nexclude = []\n")
                .expect("valid")
                .files
                .exclude,
            Vec::<String>::new()
        );
        assert_eq!(parse(b"[files]\n").expect("valid"), Config::default());
        assert_eq!(
            parse(VALID.as_bytes()).expect("valid").files,
            Files::default()
        );
    }

    #[test]
    fn files_table_errors() {
        let unknown = error("[files]\ninclude = [\"src\"]\n");
        assert!(unknown.contains("include"), "{unknown}");
        let wrong_type = error("[files]\nexclude = \"src\"\n");
        assert!(wrong_type.starts_with("line 2"), "{wrong_type}");
    }

    #[test]
    fn not_utf8() {
        assert_eq!(
            parse(b"name = \"\xff\"").expect_err("invalid").to_string(),
            "not valid UTF-8"
        );
    }
}
