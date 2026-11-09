//! TypeScript and TSX adapter, built on `tree-sitter-typescript`.
//!
//! One walk over the syntax tree finds the function units, gives each a
//! qualified name and counts its decision points (design D3 of the
//! `add-cyclomatic-complexity-check` change). What counts as a decision
//! point follows the `complexity` rule of `ESLint`.

use std::collections::HashMap;

use tree_sitter::{Node, Parser};

use crate::{FileModel, Function, Language, ParseError};

/// Parse TypeScript or TSX source into a [`FileModel`].
pub(crate) fn parse(language: Language, source: &str) -> Result<FileModel, ParseError> {
    let grammar = match language {
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX,
    };
    let mut parser = Parser::new();
    parser
        .set_language(&grammar.into())
        .map_err(|e| ParseError::Parser {
            message: e.to_string(),
        })?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| ParseError::Parser {
            message: "tree-sitter returned no tree".to_owned(),
        })?;

    let mut walker = Walker::new(source.as_bytes());
    walker.walk(tree.root_node());
    Ok(FileModel {
        functions: walker.finish(),
    })
}

/// Nodes that define a function unit, if they get a name.
const FUNCTION_KINDS: &[&str] = &[
    "function_declaration",
    "generator_function_declaration",
    "function_expression",
    "generator_function",
    "arrow_function",
    "method_definition",
];

/// Nodes that add a name segment (if named) but are not units themselves.
const CONTAINER_KINDS: &[&str] = &[
    "class_declaration",
    "abstract_class_declaration",
    "class",
    "object",
];

/// Type-level subtrees: never contain runtime decision points, so the walk
/// skips them. This keeps conditional types and the like out of the count.
const TYPE_KINDS: &[&str] = &[
    "type_annotation",
    "asserts_annotation",
    "type_predicate_annotation",
    "type_alias_declaration",
    "interface_declaration",
    "type_arguments",
    "type_parameters",
    "conditional_type",
];

/// Decision points that count once per node, whatever their content.
const DECISION_KINDS: &[&str] = &[
    "if_statement",
    "for_statement",
    "for_in_statement",
    "while_statement",
    "do_statement",
    "switch_case",
    "catch_clause",
    "ternary_expression",
    "optional_chain",
    "assignment_pattern",
    "object_assignment_pattern",
];

/// What entering a node pushed, so leaving it can pop the same.
#[derive(Default)]
struct Frame {
    segment: bool,
    scope: bool,
}

struct Walker<'s> {
    source: &'s [u8],
    /// Names of the named ancestors, joined with `.` for qualified names.
    segments: Vec<String>,
    /// Innermost scope last. `Some(i)` collects decision points into
    /// `functions[i]`; `None` is a class body, whose non-function code
    /// (field initializers, static blocks) is not measured.
    scopes: Vec<Option<usize>>,
    /// Units in source order (by start position).
    functions: Vec<Function>,
}

impl<'s> Walker<'s> {
    fn new(source: &'s [u8]) -> Self {
        Self {
            source,
            segments: Vec::new(),
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
            let node = cursor.node();
            frames.push(self.enter(node));
            if !TYPE_KINDS.contains(&node.kind()) && cursor.goto_first_child() {
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
        let kind = node.kind();
        let mut frame = Frame::default();

        if FUNCTION_KINDS.contains(&kind) {
            // Anonymous functions open nothing: their decision points go to
            // the enclosing unit.
            if let Some((name, line)) = self.name_of(node) {
                self.segments.push(name);
                self.functions.push(Function {
                    name: self.segments.join("."),
                    line,
                    cyclomatic: 1,
                });
                self.scopes.push(Some(self.functions.len() - 1));
                frame.segment = true;
                frame.scope = true;
            }
        } else if CONTAINER_KINDS.contains(&kind) {
            if let Some((name, _)) = self.name_of(node) {
                self.segments.push(name);
                frame.segment = true;
            }
        } else if kind == "class_body" {
            self.scopes.push(None);
            frame.scope = true;
        } else if is_decision_point(node) {
            if let Some(Some(index)) = self.scopes.last() {
                self.functions[*index].cyclomatic += 1;
            }
        }
        frame
    }

    fn leave(&mut self, frame: &Frame) {
        if frame.segment {
            self.segments.pop();
        }
        if frame.scope {
            self.scopes.pop();
        }
    }

    /// The name a function or container gets, and the 1-based line it
    /// appears on. `None` means the node is anonymous.
    fn name_of(&self, node: Node<'_>) -> Option<(String, u32)> {
        let kind = node.kind();

        // Declarations and methods are named by themselves.
        match kind {
            "function_declaration"
            | "generator_function_declaration"
            | "class_declaration"
            | "abstract_class_declaration" => {
                return node
                    .child_by_field_name("name")
                    .map(|n| (self.text(n), line_of(n)));
            }
            "method_definition" => {
                let name = node.child_by_field_name("name")?;
                return Some((self.method_name(node, name), line_of(name)));
            }
            _ => {}
        }

        // Expressions are named by what they are assigned to, then by
        // their own name (`function g() {}` passed as an argument).
        if let Some(named) = self.binding_name(node) {
            return Some(named);
        }
        node.child_by_field_name("name")
            .map(|n| (self.text(n), line_of(n)))
    }

    /// Name from the variable, class field or object property the node is
    /// the value of, or `default` for an unnamed default export.
    fn binding_name(&self, node: Node<'_>) -> Option<(String, u32)> {
        let parent = node.parent()?;
        let is_value = parent
            .child_by_field_name("value")
            .is_some_and(|v| v.id() == node.id());
        if !is_value {
            return None;
        }
        match parent.kind() {
            "variable_declarator" => {
                let name = parent.child_by_field_name("name")?;
                // Destructuring (`const { a } = ...`) gives no single name.
                (name.kind() == "identifier").then(|| (self.text(name), line_of(name)))
            }
            "public_field_definition" => {
                let name = parent.child_by_field_name("name")?;
                Some((self.property_name(name), line_of(name)))
            }
            "pair" => {
                let key = parent.child_by_field_name("key")?;
                Some((self.property_name(key), line_of(key)))
            }
            "export_statement" if node.child_by_field_name("name").is_none() => {
                Some(("default".to_owned(), line_of(node)))
            }
            _ => None,
        }
    }

    /// `total`, or `get size` / `set size` for accessors.
    fn method_name(&self, method: Node<'_>, name: Node<'_>) -> String {
        let base = self.property_name(name);
        let mut cursor = method.walk();
        let accessor = method
            .children(&mut cursor)
            .find(|c| !c.is_named() && matches!(c.kind(), "get" | "set"));
        match accessor {
            Some(a) => format!("{} {base}", a.kind()),
            None => base,
        }
    }

    /// Property names as written, except that quotes around string keys
    /// are dropped. Computed names keep their brackets.
    fn property_name(&self, name: Node<'_>) -> String {
        let text = self.text(name);
        if name.kind() == "string" {
            text.trim_matches(|c| c == '"' || c == '\'').to_owned()
        } else {
            text
        }
    }

    fn text(&self, node: Node<'_>) -> String {
        node.utf8_text(self.source).unwrap_or_default().to_owned()
    }

    /// The units, with `#2`, `#3`, ... appended to repeated names.
    fn finish(mut self) -> Vec<Function> {
        let mut seen: HashMap<String, u32> = HashMap::new();
        for function in &mut self.functions {
            let count = seen.entry(function.name.clone()).or_insert(0);
            *count += 1;
            if *count > 1 {
                function.name = format!("{}#{count}", function.name);
            }
        }
        self.functions
    }
}

/// Whether the node adds one to the enclosing unit's complexity.
fn is_decision_point(node: Node<'_>) -> bool {
    let kind = node.kind();
    if DECISION_KINDS.contains(&kind) {
        return true;
    }
    match kind {
        "binary_expression" => operator_in(node, &["&&", "||", "??"]),
        "augmented_assignment_expression" => operator_in(node, &["&&=", "||=", "??="]),
        // A default parameter value: `x = 1` or `{ a } = {}`.
        "required_parameter" | "optional_parameter" => node.child_by_field_name("value").is_some(),
        // Optional call `f?.()`: the grammar has no `optional_chain` node
        // here, only the bare token.
        "?." => node.parent().is_some_and(|p| p.kind() == "call_expression"),
        _ => false,
    }
}

/// Whether the node's `operator` field is one of `operators`.
fn operator_in(node: Node<'_>, operators: &[&str]) -> bool {
    node.child_by_field_name("operator")
        .is_some_and(|op| operators.contains(&op.kind()))
}

/// 1-based line of a node's start.
fn line_of(node: Node<'_>) -> u32 {
    u32::try_from(node.start_position().row + 1).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a TypeScript fixture and return `(name, cyclomatic)` pairs.
    fn complexities(source: &str) -> Vec<(String, u32)> {
        parse(Language::TypeScript, source)
            .expect("fixture parses")
            .functions
            .into_iter()
            .map(|f| (f.name, f.cyclomatic))
            .collect()
    }

    fn expected(pairs: &[(&str, u32)]) -> Vec<(String, u32)> {
        pairs.iter().map(|&(n, c)| (n.to_owned(), c)).collect()
    }

    #[test]
    fn decision_points() {
        let actual = complexities(include_str!("../fixtures/typescript/decision_points.ts"));
        assert_eq!(
            actual,
            expected(&[
                ("straight", 1),
                // if, &&, else if; the plain else does not count
                ("compoundCondition", 4),
                // for, for...in, for...of
                ("forLoops", 4),
                // while, do...while
                ("whileLoops", 3),
                // for...of, while, two cases; default does not count
                ("loopsAndSwitch", 5),
                ("onlyDefault", 1),
                // catch, ternary, ??; try and finally do not count
                ("errorHandling", 4),
                ("catchWithoutBinding", 2),
                // ||, &&, ??
                ("logicalOperators", 4),
                // ||=, &&=, ??=
                ("logicalAssignment", 4),
                // default parameter value, two ?.
                ("optionalAndDefaults", 4),
                // optional call ?.()
                ("optionalCall", 2),
                // three default values in destructuring patterns
                ("destructuringDefaults", 4),
                // default inside the pattern, default for the parameter
                ("parameterDestructuringDefault", 3),
                // conditional type, optional parameter, non-null assertion
                ("typeLevel", 1),
                // optional property, union types, assertion signature
                ("inlineTypes", 1),
            ])
        );
    }
}
