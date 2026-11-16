//! Rust adapter, built on `tree-sitter-rust` (design of
//! `add-rust-analysis`).
//!
//! One walk over the syntax tree finds the function units and names them
//! by their inline modules, their `impl` or trait and their own name
//! (design D2): `Cart.total`, `<Cart as Display>.fmt`. The same walk
//! counts decision points (D3) and records the normalized tokens for
//! finding copies (D5). Macro arguments and `macro_rules!` bodies are
//! token trees: they add tokens, but no units and no decision points (D4).
//! The walk, the unit bookkeeping and the tokens are the shared ones of
//! `common.rs`.

use std::hash::Hash;

use tree_sitter::Node;

use crate::common::{Mark, Tokens, Units, Visit, compact, line_of, node_text, run};
use crate::{FileModel, ParseError};

/// Parse Rust source into a [`FileModel`].
pub(crate) fn parse(source: &str) -> Result<FileModel, ParseError> {
    let walker = Walker::new(source.as_bytes());
    run(&tree_sitter_rust::LANGUAGE.into(), source, walker)
}

/// Literals that become one `$lit` each, without entering them. `true`
/// and `false` are keywords and keep their text.
const LITERAL_KINDS: &[&str] = &[
    "string_literal",
    "raw_string_literal",
    "char_literal",
    "integer_literal",
    "float_literal",
];

/// Subtrees that produce no tokens (design D5): they repeat across files
/// without being copied logic.
const NO_TOKEN_KINDS: &[&str] = &[
    "use_declaration",
    "extern_crate_declaration",
    "attribute_item",
    "inner_attribute_item",
    "line_comment",
    "block_comment",
];

/// What entering a node opened, so leaving it can close the same.
struct Frame {
    units: Mark,
    muted: bool,
    comment: bool,
}

struct Walker<'s> {
    source: &'s [u8],
    /// The units, named by the enclosing inline modules, impls, traits and
    /// units; a scope is the unit that collects decision points, or `None`
    /// where code is not measured.
    units: Units<Option<usize>>,
    /// Whether the walk is inside a comment, which is not code of a unit.
    in_comment: bool,
    tokens: Tokens,
}

impl<'s> Walker<'s> {
    fn new(source: &'s [u8]) -> Self {
        Self {
            source,
            units: Units::new(),
            in_comment: false,
            tokens: Tokens::default(),
        }
    }
}

impl From<Walker<'_>> for FileModel {
    fn from(walker: Walker<'_>) -> Self {
        Self {
            functions: walker.units.finish(),
            tokens: walker.tokens.finish(),
            ..Self::default()
        }
    }
}

impl Visit for Walker<'_> {
    type Frame = Frame;

    fn enter(&mut self, node: Node<'_>, ancestors: &[Node<'_>]) -> Frame {
        let parent = ancestors.last().copied();
        let mut frame = Frame {
            units: self.units.mark(),
            muted: false,
            comment: false,
        };
        self.record_token(node, parent, &mut frame);
        self.record_code(node, &mut frame);
        match node.kind() {
            // An inline module is part of the name; `mod x;` has no body
            // and names nothing here.
            "mod_item" | "trait_item" if node.child_by_field_name("body").is_some() => {
                if let Some(name) = node.child_by_field_name("name") {
                    self.units.push_segment(self.text(name));
                }
                self.units.push_scope(None);
            }
            "impl_item" => {
                let ty = node
                    .child_by_field_name("type")
                    .map(|t| compact(&self.text(t)))
                    .unwrap_or_default();
                let segment = match node.child_by_field_name("trait") {
                    Some(t) => format!("<{ty} as {}>", compact(&self.text(t))),
                    None => ty,
                };
                self.units.push_segment(segment);
                self.units.push_scope(None);
            }
            // A `fn` without a body is a `function_signature_item`.
            "function_item" => {
                if let Some(name) = node.child_by_field_name("name") {
                    let unit = self.units.open(&self.text(name), line_of(name));
                    self.units.push_scope(Some(unit));
                }
            }
            _ => self.units.count(decision_points(node, parent)),
        }
        frame
    }

    fn leave(&mut self, frame: Frame) {
        self.units.restore(frame.units);
        if frame.muted {
            self.tokens.unmute();
        }
        if frame.comment {
            self.in_comment = false;
        }
    }
}

impl Walker<'_> {
    /// Add a leaf to the fingerprint of the unit it belongs to (design D1
    /// of `add-test-gap-analysis`): all code but comments, doc comments
    /// included, and semicolons. A doc comment is a subtree of its own.
    fn record_code(&mut self, node: Node<'_>, frame: &mut Frame) {
        if matches!(node.kind(), "line_comment" | "block_comment") && !self.in_comment {
            self.in_comment = true;
            frame.comment = true;
        }
        let Some(unit) = self.units.unit() else {
            return;
        };
        let code = &self.source[node.byte_range()];
        if node.child_count() == 0 && !code.is_empty() && !self.in_comment && node.kind() != ";" {
            code.hash(self.units.hasher(unit));
        }
    }

    /// Record the normalized token `node` is, if any (design D5), and
    /// mute the subtrees that produce none.
    fn record_token(&mut self, node: Node<'_>, parent: Option<Node<'_>>, frame: &mut Frame) {
        if self.tokens.muted() {
            return;
        }
        let kind = node.kind();
        if produces_no_tokens(node, parent) {
            self.tokens.mute(&mut frame.muted);
        } else if kind == "integer_literal" && follows_path_or_dot(node) {
            // A tuple field, `.0`, is a field name (review of PR 1).
            self.tokens.push(self.text(node), node, false);
        } else if LITERAL_KINDS.contains(&kind) || matches!(kind, "lifetime" | "label") {
            // A lifetime or a loop label is a name: `'a` and `'b` are the
            // same code.
            let text = if LITERAL_KINDS.contains(&kind) {
                "$lit"
            } else {
                "$id"
            };
            self.tokens.push(text.to_owned(), node, false);
            self.tokens.mute(&mut frame.muted);
        } else if node.child_count() == 0 && kind != ";" {
            let text = self.text(node);
            let text = if is_name(kind) && text != "Self" && !follows_path_or_dot(node) {
                "$id".to_owned()
            } else {
                text
            };
            if !text.is_empty() {
                self.tokens.push(text, node, false);
            }
        }
    }

    fn text(&self, node: Node<'_>) -> String {
        node_text(node, self.source)
    }
}

/// Subtrees that produce no tokens: those of [`NO_TOKEN_KINDS`], a module
/// declared without a body, and the keyword and name of an inline module.
fn produces_no_tokens(node: Node<'_>, parent: Option<Node<'_>>) -> bool {
    let kind = node.kind();
    if NO_TOKEN_KINDS.contains(&kind) {
        return true;
    }
    if kind == "mod_item" {
        return node.child_by_field_name("body").is_none();
    }
    parent.is_some_and(|p| {
        p.kind() == "mod_item"
            && (kind == "mod"
                || p.child_by_field_name("name")
                    .is_some_and(|name| name.id() == node.id()))
    })
}

/// Leaves that are names and become `$id`: identifiers, type, field and
/// shorthand field names, and macro metavariables (`$e`).
fn is_name(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "type_identifier"
            | "field_identifier"
            | "shorthand_field_identifier"
            | "metavariable"
    )
}

/// Whether the node comes right after `.` or `::`: a field or method name,
/// or a path segment, which keeps its text (design D5). Inside a macro
/// call, where the grammar builds no paths, the previous token decides.
/// Comments in between do not count.
fn follows_path_or_dot(node: Node<'_>) -> bool {
    let mut previous = node.prev_sibling();
    while let Some(p) = previous.filter(Node::is_extra) {
        previous = p.prev_sibling();
    }
    previous.is_some_and(|p| matches!(p.kind(), "." | "::"))
}

/// How much the node adds to the enclosing unit's complexity (design D3).
/// Macro arguments are token trees, not expressions, so nothing in them
/// counts (D4).
fn decision_points(node: Node<'_>, parent: Option<Node<'_>>) -> u32 {
    let parent_kind = parent.map(|p| p.kind());
    match node.kind() {
        // `if let` and `while let` are these too, with a let condition.
        "if_expression" | "while_expression" | "for_expression" => 1,
        "let_declaration" => u32::from(node.child_by_field_name("alternative").is_some()),
        // An arm counts unless its pattern is the wildcard `_`, which the
        // grammar gives no named node; its guard is not its pattern.
        "match_pattern" => {
            let condition = node.child_by_field_name("condition").map(|c| c.id());
            let mut cursor = node.walk();
            let pattern = node
                .named_children(&mut cursor)
                .any(|c| Some(c.id()) != condition && !c.is_extra());
            u32::from(pattern) + u32::from(condition.is_some())
        }
        "binary_expression" => u32::from(
            node.child_by_field_name("operator")
                .is_some_and(|op| matches!(op.kind(), "&&" | "||")),
        ),
        // The `&&` of a let chain is no binary expression.
        "&&" => u32::from(parent_kind == Some("let_chain")),
        // A `|` between two alternatives; a leading `|` separates nothing.
        "|" => u32::from(parent_kind == Some("or_pattern") && node.prev_sibling().is_some()),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a Rust fixture and return `(name, line)` pairs.
    fn names(source: &str) -> Vec<(String, u32)> {
        parse(source)
            .expect("fixture parses")
            .functions
            .into_iter()
            .map(|f| (f.name, f.line))
            .collect()
    }

    fn pairs(items: &[(&str, u32)]) -> Vec<(String, u32)> {
        items.iter().map(|&(n, l)| (n.to_owned(), l)).collect()
    }

    // Requirements: Rust function units, Rust closures count towards their
    // named ancestor, Rust qualified names.
    #[test]
    fn units_and_their_names() {
        assert_eq!(
            names(include_str!("../fixtures/rust/units.rs")),
            pairs(&[
                ("free", 4),
                ("free.nested", 5),
                ("Cart.new", 14),
                ("Cart.total", 18),
                ("Cart.total.helper", 19),
                // the declared trait method has no body; the default one is
                // a unit
                ("Shape.area", 28),
                // the `extern` function and the closure of the static
                // initializer are not units
                ("konst", 39),
                ("shop.Cart.total", 47),
                ("shop.<Cart as fmt::Display>.fmt", 53),
                ("shop.<Cart as fmt::Debug>.fmt", 59),
                // types and traits as written, without whitespace
                ("Stack<T>.push", 66),
                ("<Id as From<u8>>.from", 70),
                ("<Id as From<HashMap<String,u32>>>.from", 76),
                ("Parser<'a>.parse", 82),
                ("<&Cart as Total>.total", 88),
                ("<Vec<T> as Total>.sum", 97),
                ("outer", 102),
                // an `impl` in a function body, a `fn` in a closure
                ("outer.Local.inside", 105),
                ("outer.in_closure", 108),
                ("home", 114),
                ("home#2", 119),
                // `macro_rules!` is not a unit
                ("tests.adds_two", 134),
                ("tests.deeper.helper", 137),
            ])
        );
    }

    /// Parse a Rust fixture and return `(name, cyclomatic)` pairs.
    fn complexities(source: &str) -> Vec<(String, u32)> {
        parse(source)
            .expect("fixture parses")
            .functions
            .into_iter()
            .map(|f| (f.name, f.cyclomatic))
            .collect()
    }

    // Requirements: Rust decision points, Macros in Rust, Rust closures
    // count towards their named ancestor.
    #[test]
    fn decision_points() {
        assert_eq!(
            complexities(include_str!("../fixtures/rust/decision_points.rs")),
            pairs(&[
                // scenario "A plain source file"
                ("plain", 2),
                // two `if`; `else` does not count
                ("if_else_if", 3),
                ("if_let", 2),
                ("while_loops", 3),
                ("for_loop", 2),
                ("let_else", 2),
                // scenario "Match": two arms and a guard, not the `_` arm
                ("arms", 4),
                // a binding arm counts
                ("binding_arm", 3),
                // a guard on `_` counts, the `_` arm does not
                ("wildcard_guard", 2),
                // scenario "Alternatives in a pattern"
                ("alternatives", 3),
                // a leading `|` separates nothing
                ("leading_vert", 4),
                ("nested_alternatives", 3),
                ("and_or", 3),
                // `if`, and two `&&` of a let chain
                ("let_chain", 4),
                // scenario "The question mark does not count"
                ("question_mark", 1),
                // `loop`, `break` and `continue` do not count
                ("loops_without_conditions", 1),
                // scenario "Let else and loops"
                ("let_else_and_loops", 4),
                // scenario "An iterator chain in a function"
                ("paid", 2),
                // a closure's `if` counts towards its function; an empty
                // closure's `||` is not an operator
                ("closures", 2),
                ("outer", 2),
                ("outer.inner", 2),
                // scenario "A condition inside an assertion": macro
                // arguments are not code
                ("in_macros", 1),
                // a `const` inside a function is its code (review of PR 1)
                ("const_inside", 2),
            ])
        );
    }

    /// The normalized token texts of `source`.
    fn tokens(source: &str) -> Vec<String> {
        parse(source)
            .expect("source parses")
            .tokens
            .into_iter()
            .map(|t| t.text)
            .collect()
    }

    fn texts(line: &str) -> Vec<String> {
        if line.is_empty() {
            return Vec::new();
        }
        line.split(' ').map(str::to_owned).collect()
    }

    // Requirement: Rust normalized tokens, "Renamed copy".
    #[test]
    fn a_renamed_copy_has_the_same_tokens() {
        let a = "fn fmt_date(d: &Date) -> String { let y = d.year; format!(\"{y}-\") }";
        let b = "fn format_order_date(when: &Date) -> String { let year = when.year; format!(\"{year}/\") }";
        assert_eq!(tokens(a), tokens(b));
        assert_eq!(
            tokens(a),
            texts("fn $id ( $id : & $id ) -> $id { let $id = $id . year $id ! ( $lit ) }")
        );
    }

    // Requirement: Rust normalized tokens, "Different members and paths".
    #[test]
    fn members_and_path_segments_keep_their_text() {
        let year = tokens("fn f(d: D) -> Vec<u32> { let v = Vec::new(); d.year }");
        let month = tokens("fn f(d: D) -> Vec<u32> { let v = Vec::new(); d.month }");
        let default = tokens("fn f(d: D) -> Vec<u32> { let v = Vec::default(); d.year }");
        assert_ne!(year, month);
        assert_ne!(year, default);
        assert_eq!(
            year,
            texts("fn $id ( $id : $id ) -> $id < u32 > { let $id = $id :: new ( ) $id . year }")
        );
    }

    #[test]
    fn each_literal_is_one_placeholder() {
        let source =
            "fn f() { (b\"x\", br#\"y\"#, 'c', b'd', 1.5, 0x1F, \"e\\t\", r\"raw\", true, false) }";
        assert_eq!(
            tokens(source),
            texts(
                "fn $id ( ) { ( $lit , $lit , $lit , $lit , $lit , $lit , $lit , $lit , true , false ) }"
            )
        );
    }

    // Requirement: Rust normalized tokens: keywords, primitive types,
    // lifetimes and raw identifiers.
    #[test]
    fn keywords_lifetimes_and_raw_identifiers() {
        let source = "impl<'a> Parser<'a> { fn new(r#type: &'a str) -> Self { Self::make(crate::X, super::Y, self.z, r#type) } }";
        assert_eq!(
            tokens(source),
            texts(
                "impl < $id > $id < $id > { fn $id ( $id : & $id str ) -> Self { Self :: make ( crate :: X , super :: Y , self . z , $id ) } }"
            )
        );
    }

    // Requirement: Macros in Rust, "Copies inside macro calls".
    #[test]
    fn macro_calls_are_normalized_like_code() {
        let a = tokens("fn a(x: u32) { println!(\"{} {}\", x, y.len()); }");
        let b = tokens("fn b(n: u32) { println!(\"{}-{}\", n, m.len()); }");
        assert_eq!(a, b);
        assert_eq!(
            a,
            texts("fn $id ( $id : u32 ) { $id ! ( $lit , $id , $id . len ( ) ) }")
        );
        // A path inside a macro call keeps its segments, too.
        assert_ne!(
            tokens("fn f() { v!(Vec::new()) }"),
            tokens("fn f() { v!(Vec::default()) }")
        );
    }

    // Requirement: Macros in Rust: a definition keeps its tokens. The
    // grammar reads `macro_rules!` as one token.
    #[test]
    fn macro_definitions_keep_their_tokens() {
        assert_eq!(
            tokens("macro_rules! twice { ($e:expr) => { $e + $e }; }"),
            texts("macro_rules! $id { ( $id : expr ) => { $id + $id } }")
        );
    }

    // Requirement: Rust code without tokens, "Imports and attributes are
    // not compared".
    #[test]
    fn imports_attributes_and_comments_have_no_tokens() {
        let source = "#![allow(dead_code)]\nuse std::fmt;\nuse super::*;\nextern crate alloc;\n\
                      mod a;\npub mod b;\n/// doc\n#[derive(Debug, Clone)]\n// line\n/* block */\n\
                      struct S;\nmod tests { use super::*; #[test] fn f() {} }\n";
        assert_eq!(tokens(source), texts("struct $id { fn $id ( ) { } }"));
        assert_eq!(
            tokens("use std::fmt;\nuse crate::a::{b, c};\nmod x;\n#![deny(missing_docs)]\n"),
            texts("")
        );
    }

    // Requirement: Rust code without tokens, "Doc comments".
    #[test]
    fn doc_comments_have_no_tokens() {
        assert_eq!(
            tokens("/// Adds one.\n/** More. */\nfn f(x: u32) -> u32 { x + 1 }"),
            tokens("/// Increments.\nfn f(x: u32) -> u32 { x + 1 }")
        );
    }

    // Requirement: Rust files, "A file with a byte order mark".
    #[test]
    fn a_byte_order_mark_is_ignored() {
        let model = parse("\u{feff}fn f(a: bool) { if a {} }").expect("parses");
        assert_eq!(model.functions[0].cyclomatic, 2);
        assert_eq!(model.tokens[0].text, "fn");
    }

    // Review of PR 1: a comment between `.` and a member is not the
    // previous token; a loop label is one name, like a lifetime.
    #[test]
    fn comments_after_a_dot_and_loop_labels() {
        assert_eq!(
            tokens("fn f() { x. /* c */ year; v!(x. /* c */ year) }"),
            texts("fn $id ( ) { $id . year $id ! ( $id . year ) }")
        );
        // A tuple field is a field name (review of PR 1).
        assert_ne!(
            tokens("fn f() { t.0.len() }"),
            tokens("fn f() { t.1.len() }")
        );
        assert_eq!(
            tokens("fn f() { self.0.len() }"),
            texts("fn $id ( ) { self . 0 . len ( ) }")
        );
        assert_eq!(
            tokens("fn f() { 'outer: loop { break 'outer; } }"),
            texts("fn $id ( ) { $id : loop { break $id } }")
        );
    }

    #[test]
    fn tokens_carry_their_line() {
        let model = parse("fn f()\n{\n    1\n}\n").expect("parses");
        let lines: Vec<u32> = model.tokens.iter().map(|t| t.line).collect();
        assert_eq!(lines, [1, 1, 1, 1, 2, 3, 4]);
    }

    // Design D1 of `add-test-gap-analysis`: layout and comments (doc
    // comments too) are not changes; literals, names, types, closures and
    // macro arguments are; a nested function's change is its own, and an
    // attribute before a function or a static's initializer belongs to no
    // unit.
    #[test]
    fn fingerprints_tell_changed_code() {
        let fingerprints = |source: &str| -> Vec<(String, u64)> {
            parse(source)
                .expect("fixture parses")
                .functions
                .into_iter()
                .map(|f| (f.name, f.fingerprint))
                .collect()
        };
        let before = fingerprints(include_str!("../fixtures/rust/fingerprints/before.rs"));
        let after = fingerprints(include_str!("../fixtures/rust/fingerprints/after.rs"));
        let names = |units: &[(String, u64)]| units.iter().map(|u| u.0.clone()).collect::<Vec<_>>();
        assert_eq!(names(&before), names(&after), "the same units");
        let changed: Vec<String> = before
            .into_iter()
            .zip(after)
            .filter(|(b, a)| b.1 != a.1)
            .map(|(b, _)| b.0)
            .collect();
        assert_eq!(
            changed,
            [
                "literal",
                "renamed",
                "typed",
                "outer.inner",
                "closure",
                "in_macro",
                "local_const",
            ]
        );
    }

    // The grammar lags behind the language (README "Rust"): it does not
    // read the edition-2024 `safe` qualifier. If this starts to parse,
    // update the README.
    #[test]
    fn the_grammar_does_not_know_safe_items() {
        assert_eq!(
            parse("unsafe extern \"C\" {\n    safe fn sqrt(x: f64) -> f64;\n}\n"),
            Err(ParseError::Syntax { line: 2 })
        );
    }

    #[test]
    fn a_syntax_error_names_its_line() {
        assert_eq!(
            parse("fn f() {\n    let x = ;\n}\n"),
            Err(ParseError::Syntax { line: 2 })
        );
    }

    // Requirement: Rust function units, "Declarations without a body".
    #[test]
    fn declarations_without_a_body() {
        let source = "trait T { fn a(&self); fn b(&self) -> u32; }\n\
                      extern \"C\" { fn abs(x: i32) -> i32; }\n";
        assert_eq!(names(source), pairs(&[]));
    }
}
