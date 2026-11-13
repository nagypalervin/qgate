//! C# adapter, built on `tree-sitter-c-sharp` (design of
//! `add-csharp-analysis`).
//!
//! One walk over the syntax tree finds the function units and names them
//! by their enclosing types and their own signature (design D2), so an
//! overload keeps its name when another one is added above it.

use std::collections::HashSet;

use tree_sitter::Node;

use crate::common::{line_of, parse_tree, unique_segment};
use crate::{FileModel, Function, ParseError};

/// Parse C# source into a [`FileModel`].
pub(crate) fn parse(source: &str) -> Result<FileModel, ParseError> {
    let tree = parse_tree(&tree_sitter_c_sharp::LANGUAGE.into(), source)?;
    let mut walker = Walker::new(source.as_bytes());
    walker.walk(tree.root_node());
    Ok(FileModel {
        functions: walker.functions,
        ..FileModel::default()
    })
}

/// Type declarations: they add their name to the units inside them, and
/// the code directly in them (field initializers) is not measured.
const TYPE_KINDS: &[&str] = &[
    "class_declaration",
    "struct_declaration",
    "record_declaration",
    "record_struct_declaration",
    "interface_declaration",
];

/// Members that are a unit when they have a body or an expression body.
const UNIT_KINDS: &[&str] = &[
    "method_declaration",
    "local_function_statement",
    "constructor_declaration",
    "destructor_declaration",
    "operator_declaration",
    "conversion_operator_declaration",
];

/// Members whose accessors are units, named after them.
const ACCESSOR_OWNER_KINDS: &[&str] = &[
    "property_declaration",
    "indexer_declaration",
    "event_declaration",
];

/// Decision points that count once per node, whatever their content
/// (design D3).
const DECISION_KINDS: &[&str] = &[
    "if_statement",
    "for_statement",
    "foreach_statement",
    "while_statement",
    "do_statement",
    "catch_clause",
    "catch_filter_clause",
    "when_clause",
    "conditional_expression",
    "conditional_access_expression",
    "and_pattern",
    "or_pattern",
];

/// What entering a node pushed, so leaving it can pop the same. An
/// expression-bodied property pushes two segments: its name and `get`.
#[derive(Default)]
struct Frame {
    segments: usize,
    scopes: usize,
}

struct Walker<'s> {
    source: &'s [u8],
    /// Names of the enclosing types, members and units.
    segments: Vec<String>,
    /// Qualified names already given to a unit.
    taken: HashSet<String>,
    /// Innermost last: the unit that collects decision points, or `None`
    /// in a type body, where code is not measured. Empty at top level.
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
        let kind = node.kind();
        let mut frame = Frame::default();
        if TYPE_KINDS.contains(&kind) {
            if let Some(name) = node.child_by_field_name("name") {
                self.segments.push(self.text(name));
                frame.segments += 1;
            }
            self.scopes.push(None);
            frame.scopes += 1;
        } else if ACCESSOR_OWNER_KINDS.contains(&kind) {
            self.segments.push(self.member_name(node));
            frame.segments += 1;
            // `int Count => n;` is a getter of its own.
            if node
                .child_by_field_name("value")
                .is_some_and(|v| v.kind() == "arrow_expression_clause")
            {
                self.open_unit("get", Self::member_line(node), &mut frame);
            }
        } else if UNIT_KINDS.contains(&kind) && node.child_by_field_name("body").is_some() {
            let name = self.unit_name(node);
            self.open_unit(&name, Self::member_line(node), &mut frame);
        } else if kind == "accessor_declaration" && node.child_by_field_name("body").is_some() {
            if let Some(name) = node.child_by_field_name("name") {
                self.open_unit(&self.text(name), line_of(name), &mut frame);
            }
        } else if is_decision_point(node) {
            if let Some(Some(index)) = self.scopes.last() {
                self.functions[*index].cyclomatic += 1;
            }
        }
        frame
    }

    /// Start the unit `name` inside the current segments.
    fn open_unit(&mut self, name: &str, line: u32, frame: &mut Frame) {
        let segment = unique_segment(&self.segments, &self.taken, name);
        self.segments.push(segment);
        let qualified = self.segments.join(".");
        self.taken.insert(qualified.clone());
        self.functions.push(Function {
            name: qualified,
            line,
            cyclomatic: 1,
        });
        self.scopes.push(Some(self.functions.len() - 1));
        frame.segments += 1;
        frame.scopes += 1;
    }

    fn leave(&mut self, frame: &Frame) {
        let segments = self.segments.len() - frame.segments;
        self.segments.truncate(segments);
        let scopes = self.scopes.len() - frame.scopes;
        self.scopes.truncate(scopes);
    }

    /// A unit's own name: its signature (design D2).
    fn unit_name(&self, node: Node<'_>) -> String {
        let params = node
            .child_by_field_name("parameters")
            .map(|p| self.parameter_list(p))
            .unwrap_or_default();
        let name = |field: &str| {
            node.child_by_field_name(field)
                .map(|n| self.text(n))
                .unwrap_or_default()
        };
        match node.kind() {
            "destructor_declaration" => format!("~{}({params})", name("name")),
            "operator_declaration" => format!("operator {}({params})", name("operator")),
            "conversion_operator_declaration" => {
                // `implicit` or `explicit`, as written.
                let mut cursor = node.walk();
                let direction = node
                    .children(&mut cursor)
                    .find(|c| matches!(c.kind(), "implicit" | "explicit"))
                    .map(|c| c.kind())
                    .unwrap_or_default();
                format!("{direction} operator {}({params})", compact(&name("type")))
            }
            _ => {
                // Methods, local functions and constructors; a method can
                // implement an interface member explicitly.
                let interface = node
                    .named_children(&mut node.walk())
                    .find(|c| c.kind() == "explicit_interface_specifier")
                    .map(|c| compact(&self.text(c)))
                    .unwrap_or_default();
                let type_parameters = node
                    .child_by_field_name("type_parameters")
                    .map(|t| compact(&self.text(t)))
                    .unwrap_or_default();
                format!("{interface}{}{type_parameters}({params})", name("name"))
            }
        }
    }

    /// The name of a property, event or indexer: `this[int]` for an
    /// indexer.
    fn member_name(&self, node: Node<'_>) -> String {
        if node.kind() == "indexer_declaration" {
            let params = node
                .child_by_field_name("parameters")
                .map(|p| self.parameter_list(p))
                .unwrap_or_default();
            return format!("this[{params}]");
        }
        node.child_by_field_name("name")
            .map(|n| self.text(n))
            .unwrap_or_default()
    }

    /// The line of a member's name, or of `operator` or `this` for one
    /// without a name.
    fn member_line(node: Node<'_>) -> u32 {
        if let Some(name) = node.child_by_field_name("name") {
            return line_of(name);
        }
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|c| matches!(c.kind(), "operator" | "this"))
            .map_or_else(|| line_of(node), line_of)
    }

    /// The parameters' modifiers and types, joined by `, `, without names
    /// or default values (design D2). The grammar puts a `params`
    /// parameter's keyword and type directly in the list.
    fn parameter_list(&self, list: Node<'_>) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut cursor = list.walk();
        let mut params = false;
        for (i, child) in list.children(&mut cursor).enumerate() {
            let field = u32::try_from(i)
                .ok()
                .and_then(|i| list.field_name_for_child(i));
            match child.kind() {
                "parameter" => parts.push(self.parameter(child)),
                "params" => params = true,
                _ if params && field == Some("type") => {
                    parts.push(format!("params {}", compact(&self.text(child))));
                    params = false;
                }
                _ => {}
            }
        }
        parts.join(", ")
    }

    /// `ref T`, `this string?`: the modifiers and the type of a parameter.
    fn parameter(&self, parameter: Node<'_>) -> String {
        let mut words: Vec<String> = Vec::new();
        let mut cursor = parameter.walk();
        for child in parameter.children(&mut cursor) {
            if child.kind() == "modifier" {
                words.push(self.text(child));
            }
        }
        if let Some(ty) = parameter.child_by_field_name("type") {
            words.push(compact(&self.text(ty)));
        }
        words.join(" ")
    }

    fn text(&self, node: Node<'_>) -> String {
        node.utf8_text(self.source).unwrap_or_default().to_owned()
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
        "assignment_expression" => operator_in(node, &["??="]),
        // Each `case` label of a switch statement; `default` does not
        // count, as `else` does not.
        "case" => !node.is_named() && node.parent().is_some_and(|p| p.kind() == "switch_section"),
        // An arm counts unless its pattern is the discard `_`.
        "switch_expression_arm" => node.named_child(0).is_some_and(|p| p.kind() != "discard"),
        _ => false,
    }
}

/// Whether the node's `operator` field is one of `operators`.
fn operator_in(node: Node<'_>, operators: &[&str]) -> bool {
    node.child_by_field_name("operator")
        .is_some_and(|op| operators.contains(&op.kind()))
}

/// `text` without whitespace, except one space between two words:
/// `Dictionary<string, int>` becomes `Dictionary<string,int>`, and
/// `scoped Span<int>` stays as it is.
fn compact(text: &str) -> String {
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '@';
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars() {
        if c.is_whitespace() {
            space = true;
            continue;
        }
        if space && out.chars().next_back().is_some_and(word) && word(c) {
            out.push(' ');
        }
        space = false;
        out.push(c);
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a C# fixture and return `(name, line)` pairs.
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

    // Requirements: C# function units, C# qualified names.
    #[test]
    fn units_and_their_names() {
        assert_eq!(
            names(include_str!("../fixtures/csharp/units.cs")),
            pairs(&[
                ("Cart.Cart(IClock)", 9),
                ("Cart.Total(int)", 11),
                ("Cart.Total(int).Twice(int)", 13),
                ("Cart.Owner.get", 20),
                ("Cart.Owner.set", 21),
                ("Cart.Count.get", 24),
                ("Cart.operator +(Cart, Cart)", 26),
                // interface and abstract members without a body are not
                // units; a default interface method is
                ("IShop.Log()", 37),
                // two partial declarations of one class
                ("Part.A()", 47),
                ("Part.A(int)", 48),
                ("Disposer.IDisposable.Dispose()", 52),
                // a static and an instance constructor without parameters
                ("Disposer.Disposer()", 53),
                ("Disposer.Disposer()#2", 54),
                ("Disposer.~Disposer()", 55),
                ("Disposer.Changed.add", 56),
                ("Disposer.Changed.remove", 56),
                ("Disposer.implicit operator int(Disposer)", 57),
                // the record's primary constructor has no body
                ("Money.Add(Money)", 62),
                ("Point.Length()", 67),
            ])
        );
    }

    // Requirements: C# qualified names, C# parameter lists in names.
    #[test]
    fn signatures_in_names() {
        assert_eq!(
            names(include_str!("../fixtures/csharp/names.cs")),
            pairs(&[
                ("Cart.Cart(IClock)", 8),
                ("Cart.Total(int)", 9),
                ("Cart.Owner.get", 10),
                ("Cart.Owner.init", 10),
                ("Cart.this[int].get", 11),
                ("Cart.Total(int, string)", 13),
                ("Cart.Total(string)", 14),
                ("Cart.Total(Dictionary<string,int>, out int)", 15),
                ("Cart.Paid(List<Order>)", 16),
                ("Outer.Inner.Map<T>(ref T, params int[])", 23),
                ("Outer.Inner.Map<T>(ref T, params int[]).Add(int, int)", 25),
                ("Outer.Inner.operator +(Inner, Inner)", 29),
                ("Outer.Inner.~Inner()", 31),
                ("Ext.Count(this string?, in int, scoped Span<int>)", 37),
            ])
        );
    }

    // Requirement: C# lambdas count towards their named ancestor.
    #[test]
    fn top_level_statements() {
        assert_eq!(
            names(include_str!("../fixtures/csharp/top_level.cs")),
            pairs(&[("Twice(int)", 10)])
        );
    }

    /// Parse a C# fixture and return `(name, cyclomatic)` pairs.
    fn complexities(source: &str) -> Vec<(String, u32)> {
        parse(source)
            .expect("fixture parses")
            .functions
            .into_iter()
            .map(|f| (f.name, f.cyclomatic))
            .collect()
    }

    // Requirements: C# decision points, Conditional compilation.
    #[test]
    fn decision_points() {
        assert_eq!(
            complexities(include_str!("../fixtures/csharp/decision_points.cs")),
            pairs(&[
                // two `if`; `else` does not count
                ("D.If(bool)", 3),
                ("D.For()", 2),
                ("D.Foreach(int[])", 2),
                ("D.While(int)", 2),
                ("D.Do(int)", 2),
                // three `case` labels, `default` does not count
                ("D.Cases(int)", 4),
                // a `case` and its `when`
                ("D.Guard(object)", 3),
                // two arms, the `_` arm does not count
                ("D.Arms(int)", 3),
                // two `catch` and a `when` filter; `finally` does not count
                ("D.Catch()", 4),
                ("D.Ternary(bool)", 2),
                ("D.And(bool, bool)", 2),
                ("D.Or(bool, bool)", 2),
                ("D.Coalesce(string?)", 2),
                ("D.CoalesceAssign(string?)", 2),
                ("D.Access(A?)", 3),
                ("D.Element(string?)", 2),
                ("D.AndPattern(int)", 2),
                ("D.OrPattern(int)", 3),
                // an optional parameter's value does not count
                ("D.Optional(int)", 1),
                // `not` patterns do not count; the `&&` does
                ("D.NotNull(object?)", 2),
                // a lambda counts towards its method
                ("D.Lambda(List<int>)", 2),
                ("D.Default(int)", 1),
                ("D.Discard(int)", 1),
                // `goto` does not count
                ("D.Goto(int)", 2),
                ("D.Property.get", 2),
                // `D.Field` is a field initializer: not measured
                ("D.LoopsAndSwitch(int[], int)", 5),
                ("D.F(int)", 4),
                ("D.Errors(A?)", 5),
                ("D.Guarded(bool)", 2),
            ])
        );
    }

    // Requirement: C# lambdas count towards their named ancestor.
    #[test]
    fn lambdas_and_top_level_code() {
        let names = complexities(include_str!("../fixtures/csharp/names.cs"));
        assert!(
            names.contains(&("Cart.Paid(List<Order>)".to_owned(), 2)),
            "{names:?}"
        );
        assert_eq!(
            complexities(include_str!("../fixtures/csharp/top_level.cs")),
            pairs(&[("Twice(int)", 2)])
        );
    }

    #[test]
    fn declarations_without_a_body() {
        let source = "interface I { void Open(); int Close(int c); }\n\
                      abstract class B { public abstract void Run(); partial void On(); static extern int N(); }";
        assert_eq!(names(source), pairs(&[]));
    }
}
