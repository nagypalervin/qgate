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
        loop {
            frames.push(self.enter(cursor.node()));
            if cursor.goto_first_child() {
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
            }
        }
    }

    fn enter(&mut self, node: Node<'_>) -> Frame {
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
            _ => {}
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

    // Requirement: Rust function units, "Declarations without a body".
    #[test]
    fn declarations_without_a_body() {
        let source = "trait T { fn a(&self); fn b(&self) -> u32; }\n\
                      extern \"C\" { fn abs(x: i32) -> i32; }\n";
        assert_eq!(names(source), pairs(&[]));
    }
}
