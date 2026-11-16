//! Rust adapter, built on `tree-sitter-rust` (design of
//! `add-rust-analysis`).
//!
//! One walk over the syntax tree finds the function units and names them
//! by their inline modules, their `impl` or trait and their own name
//! (design D2): `Cart.total`, `<Cart as Display>.fmt`.

use std::collections::HashSet;

use tree_sitter::Node;

use crate::common::{compact, line_of, parse_tree, unique_segment};
use crate::{FileModel, Function, ParseError};

/// Parse Rust source into a [`FileModel`].
pub(crate) fn parse(source: &str) -> Result<FileModel, ParseError> {
    let tree = parse_tree(&tree_sitter_rust::LANGUAGE.into(), source)?;
    let mut walker = Walker::new(source.as_bytes());
    walker.walk(tree.root_node());
    Ok(FileModel {
        functions: walker.functions,
        ..FileModel::default()
    })
}

/// Items whose code is not measured, even inside a function: the
/// initializers of `const` and `static` items.
const UNMEASURED_KINDS: &[&str] = &["const_item", "static_item"];

/// What entering a node pushed, so leaving it can pop the same.
#[derive(Default)]
struct Frame {
    segments: usize,
    scopes: usize,
}

struct Walker<'s> {
    source: &'s [u8],
    /// Names of the enclosing inline modules, impls, traits and units.
    segments: Vec<String>,
    /// Qualified names already given to a unit.
    taken: HashSet<String>,
    /// Innermost last: the unit that collects decision points, or `None`
    /// where code is not measured. Empty at top level.
    scopes: Vec<Option<usize>>,
    /// Units in source order.
    functions: Vec<Function>,
}

impl<'s> Walker<'s> {
    fn new(source: &'s [u8]) -> Self {
        Self {
            source,
            segments: Vec::new(),
            taken: HashSet::new(),
            scopes: Vec::new(),
            functions: Vec::new(),
        }
    }

    /// Depth-first walk without recursion, so deeply nested code cannot
    /// overflow the stack.
    fn walk(&mut self, root: Node<'_>) {
        let mut cursor = root.walk();
        let mut frames = Vec::new();
        // The ancestors of the current node, so no node has to ask
        // tree-sitter for its parent.
        let mut ancestors: Vec<Node<'_>> = Vec::new();
        loop {
            let node = cursor.node();
            frames.push(self.enter(node, ancestors.last().copied()));
            if cursor.goto_first_child() {
                ancestors.push(node);
                continue;
            }
            loop {
                if let Some(frame) = frames.pop() {
                    self.leave(&frame);
                }
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return;
                }
                ancestors.pop();
            }
        }
    }

    fn enter(&mut self, node: Node<'_>, parent: Option<Node<'_>>) -> Frame {
        let mut frame = Frame::default();
        match node.kind() {
            // An inline module is part of the name; `mod x;` has no body
            // and names nothing here.
            "mod_item" | "trait_item" if node.child_by_field_name("body").is_some() => {
                if let Some(name) = node.child_by_field_name("name") {
                    self.push_segment(self.text(name), &mut frame);
                }
                self.push_scope(None, &mut frame);
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
                self.push_segment(segment, &mut frame);
                self.push_scope(None, &mut frame);
            }
            // A `fn` without a body is a `function_signature_item`.
            "function_item" => {
                if let Some(name) = node.child_by_field_name("name") {
                    self.open_unit(&self.text(name), line_of(name), &mut frame);
                }
            }
            kind if UNMEASURED_KINDS.contains(&kind) => self.push_scope(None, &mut frame),
            _ => {
                let points = decision_points(node, parent);
                if let (Some(Some(index)), 1..) = (self.scopes.last(), points) {
                    self.functions[*index].cyclomatic += points;
                }
            }
        }
        frame
    }

    fn push_segment(&mut self, segment: String, frame: &mut Frame) {
        self.segments.push(segment);
        frame.segments += 1;
    }

    fn push_scope(&mut self, scope: Option<usize>, frame: &mut Frame) {
        self.scopes.push(scope);
        frame.scopes += 1;
    }

    /// Start the unit `name` inside the current segments.
    fn open_unit(&mut self, name: &str, line: u32, frame: &mut Frame) {
        let segment = unique_segment(&self.segments, &self.taken, name);
        self.push_segment(segment, frame);
        let qualified = self.segments.join(".");
        self.taken.insert(qualified.clone());
        self.functions.push(Function {
            name: qualified,
            line,
            cyclomatic: 1,
            fingerprint: 0,
        });
        self.push_scope(Some(self.functions.len() - 1), frame);
    }

    fn leave(&mut self, frame: &Frame) {
        let segments = self.segments.len() - frame.segments;
        self.segments.truncate(segments);
        let scopes = self.scopes.len() - frame.scopes;
        self.scopes.truncate(scopes);
    }

    fn text(&self, node: Node<'_>) -> String {
        node.utf8_text(self.source).unwrap_or_default().to_owned()
    }
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
                // a `const` initializer is not measured
                ("const_inside", 1),
            ])
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
