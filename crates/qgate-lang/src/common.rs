//! Helpers that every tree-sitter adapter needs: building the tree and
//! refusing a file with syntax errors, lines, and unique unit names
//! (design D1 of `add-csharp-analysis`).

use std::collections::HashSet;

use tree_sitter::{Language as Grammar, Node, Parser, Tree};

use crate::ParseError;

/// Parse `source` with `grammar`. A tree with a syntax error is refused
/// with the line of its first error: counting a partly parsed file could
/// silently distort the results.
pub(crate) fn parse_tree(grammar: &Grammar, source: &str) -> Result<Tree, ParseError> {
    let mut parser = Parser::new();
    parser
        .set_language(grammar)
        .map_err(|e| ParseError::Parser {
            message: e.to_string(),
        })?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| ParseError::Parser {
            message: "tree-sitter returned no tree".to_owned(),
        })?;
    match first_error_line(tree.root_node()) {
        Some(line) => Err(ParseError::Syntax { line }),
        None => Ok(tree),
    }
}

/// Line of the earliest `ERROR` or missing node, if the tree has any.
/// Only subtrees that contain an error are visited.
fn first_error_line(root: Node<'_>) -> Option<u32> {
    if !root.has_error() {
        return None;
    }
    let mut first: Option<Node<'_>> = None;
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.is_error() || node.is_missing() {
            if first.is_none_or(|f| node.start_byte() < f.start_byte()) {
                first = Some(node);
            }
            continue;
        }
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor).filter(Node::has_error));
    }
    first.map(line_of)
}

/// 1-based line of a node's start.
pub(crate) fn line_of(node: Node<'_>) -> u32 {
    u32::try_from(node.start_position().row + 1).unwrap_or(u32::MAX)
}

/// The named children of a node.
pub(crate) fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

/// `name`, or `name#2`, `name#3`, ... if the qualified name it would give
/// after `segments` is already `taken`. Nested units inherit the suffix
/// because it is part of the segment.
pub(crate) fn unique_segment(segments: &[String], taken: &HashSet<String>, name: &str) -> String {
    let qualify = |segment: &str| {
        let mut parts = segments.to_vec();
        parts.push(segment.to_owned());
        parts.join(".")
    };
    let mut segment = name.to_owned();
    let mut n = 2;
    while taken.contains(&qualify(&segment)) {
        segment = format!("{name}#{n}");
        n += 1;
    }
    segment
}
