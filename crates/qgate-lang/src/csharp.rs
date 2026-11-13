//! C# adapter, built on `tree-sitter-c-sharp` (design of
//! `add-csharp-analysis`).
//!
//! One walk over the syntax tree finds the function units and names them
//! by their enclosing types and their own signature (design D2), so an
//! overload keeps its name when another one is added above it. The same
//! walk counts decision points (D3) and records the normalized tokens
//! for finding copies (D4).

use std::collections::HashSet;

use tree_sitter::Node;

use crate::common::{line_of, parse_tree, unique_segment};
use crate::{FileModel, Function, ParseError, Token};

/// Parse C# source into a [`FileModel`].
pub(crate) fn parse(source: &str) -> Result<FileModel, ParseError> {
    let tree = parse_tree(&tree_sitter_c_sharp::LANGUAGE.into(), source)?;
    let mut walker = Walker::new(source.as_bytes());
    walker.walk(tree.root_node());
    Ok(FileModel {
        functions: walker.functions,
        imports: Vec::new(),
        tokens: walker.tokens,
    })
}

/// Type declarations: they add their name to the units inside them, and
/// the code directly in them (field initializers) is not measured.
const TYPE_KINDS: &[&str] = &[
    "class_declaration",
    "struct_declaration",
    "record_declaration",
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

/// Literals that become one `$lit` each, without entering them. A
/// format in an interpolation (`{x:D2}`) is a literal too.
const LITERAL_KINDS: &[&str] = &[
    "string_literal",
    "verbatim_string_literal",
    "raw_string_literal",
    "character_literal",
    "integer_literal",
    "real_literal",
    "interpolation_format_clause",
];

/// The text parts of an interpolated string: a run of them is one `$lit`.
const TEXT_PART_KINDS: &[&str] = &["string_content", "escape_sequence", "raw_string_content"];

/// What entering a node pushed, so leaving it can pop the same. An
/// expression-bodied property pushes two segments: its name and `get`.
#[derive(Default)]
struct Frame {
    segments: usize,
    scopes: usize,
    muted: bool,
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
    /// Normalized tokens in source order.
    tokens: Vec<Token>,
    /// Whether the walk is inside a subtree that produces no tokens; the
    /// frame of the node that opened it closes it.
    muted: bool,
    /// Whether the last token was a text part of an interpolated string,
    /// so the next one continues the same literal.
    in_text_run: bool,
}

impl<'s> Walker<'s> {
    fn new(source: &'s [u8]) -> Self {
        Self {
            source,
            segments: Vec::new(),
            taken: HashSet::new(),
            scopes: Vec::new(),
            functions: Vec::new(),
            tokens: Vec::new(),
            muted: false,
            in_text_run: false,
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
            frames.push(self.enter(node, &ancestors));
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

    /// `ancestors` are the node's ancestors, its parent last.
    fn enter(&mut self, node: Node<'_>, ancestors: &[Node<'_>]) -> Frame {
        let kind = node.kind();
        let parent = ancestors.last().copied();
        let mut frame = Frame::default();
        self.record_token(node, ancestors, &mut frame);
        if TYPE_KINDS.contains(&kind) {
            // A generic type keeps its type parameters: `Result<T>` and
            // `Result` can share a file.
            if let Some(name) = node.child_by_field_name("name") {
                // The grammar gives a type's list no field name.
                let mut cursor = node.walk();
                let type_parameters = node
                    .named_children(&mut cursor)
                    .find(|c| c.kind() == "type_parameter_list")
                    .map(|t| compact(&self.text(t)))
                    .unwrap_or_default();
                self.segments
                    .push(format!("{}{type_parameters}", self.text(name)));
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
        } else if !self.muted && is_decision_point(node, parent) {
            // Not in muted code: the condition of `#if DEBUG && TRACE` is
            // not code of the unit.
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
        if frame.muted {
            self.muted = false;
        }
    }

    /// Record the normalized token `node` is, if any (design D4), and
    /// mute the subtrees that produce none.
    fn record_token(&mut self, node: Node<'_>, ancestors: &[Node<'_>], frame: &mut Frame) {
        if self.muted {
            return;
        }
        let kind = node.kind();
        let parent = ancestors.last().copied();
        if produces_no_tokens(node, parent) {
            self.muted = true;
            frame.muted = true;
        } else if node.is_named() && LITERAL_KINDS.contains(&kind) {
            self.push_token("$lit".to_owned(), node, false);
            if node.child_count() > 0 {
                self.muted = true;
                frame.muted = true;
            }
        } else if node.is_named()
            && TEXT_PART_KINDS.contains(&kind)
            && parent.is_some_and(|p| p.kind() == "interpolated_string_expression")
        {
            if !self.in_text_run {
                self.push_token("$lit".to_owned(), node, true);
            }
        } else if kind == "identifier" {
            // An identifier is one token even when the grammar puts a
            // contextual keyword inside it (`when`, `value`).
            let text = if is_member_name(node, ancestors) {
                self.text(node)
            } else {
                "$id".to_owned()
            };
            self.push_token(text, node, false);
            if node.child_count() > 0 {
                self.muted = true;
                frame.muted = true;
            }
        } else if node.child_count() == 0 && kind != ";" && !kind.starts_with('#') {
            let text = self.text(node);
            if !text.is_empty() {
                self.push_token(text, node, false);
            }
        }
    }

    fn push_token(&mut self, text: String, node: Node<'_>, text_part: bool) {
        self.tokens.push(Token {
            text,
            line: line_of(node),
        });
        self.in_text_run = text_part;
    }

    /// A unit's own name: its signature (design D2), after the interface
    /// it implements explicitly, if any.
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
        // `operator checked +` is a member of its own.
        let checked = if has_child(node, &["checked"]) {
            "checked "
        } else {
            ""
        };
        let interface = self.interface(node);
        match node.kind() {
            "destructor_declaration" => format!("~{}({params})", name("name")),
            "operator_declaration" => {
                format!(
                    "{interface}operator {checked}{}({params})",
                    name("operator")
                )
            }
            "conversion_operator_declaration" => {
                // `implicit` or `explicit`, as written.
                let direction = if has_child(node, &["implicit"]) {
                    "implicit"
                } else {
                    "explicit"
                };
                let ty = compact(&name("type"));
                format!("{interface}{direction} operator {checked}{ty}({params})")
            }
            _ => {
                // Methods, local functions and constructors.
                let type_parameters = node
                    .child_by_field_name("type_parameters")
                    .map(|t| compact(&self.text(t)))
                    .unwrap_or_default();
                format!("{interface}{}{type_parameters}({params})", name("name"))
            }
        }
    }

    /// `I.` or `IAdd<C>.` for a member that implements an interface member
    /// explicitly, otherwise empty.
    fn interface(&self, node: Node<'_>) -> String {
        let mut cursor = node.walk();
        node.named_children(&mut cursor)
            .find(|c| c.kind() == "explicit_interface_specifier")
            .map(|c| compact(&self.text(c)))
            .unwrap_or_default()
    }

    /// The name of a property, event or indexer, after the interface it
    /// implements explicitly: `this[int]` for an indexer.
    fn member_name(&self, node: Node<'_>) -> String {
        let interface = self.interface(node);
        if node.kind() == "indexer_declaration" {
            let params = node
                .child_by_field_name("parameters")
                .map(|p| self.parameter_list(p))
                .unwrap_or_default();
            return format!("{interface}this[{params}]");
        }
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(n))
            .unwrap_or_default();
        format!("{interface}{name}")
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

/// Subtrees that produce no tokens: `using` directives, a namespace's
/// keyword and name, comments, and preprocessor directives (design D4). A
/// file-scoped namespace declaration holds nothing else. The branches of
/// an `#if` hold code, which keeps its tokens; only their conditions are
/// muted (the `#if`, `#elif`, `#else` and `#endif` tokens are skipped as
/// leaves).
fn produces_no_tokens(node: Node<'_>, parent: Option<Node<'_>>) -> bool {
    let kind = node.kind();
    if matches!(kind, "preproc_if" | "preproc_elif" | "preproc_else") {
        return false;
    }
    matches!(
        kind,
        "using_directive" | "comment" | "file_scoped_namespace_declaration"
    ) || kind.starts_with("preproc_")
        || parent.is_some_and(|p| match p.kind() {
            "namespace_declaration" => kind == "namespace" || is_field(p, "name", node),
            "preproc_if" | "preproc_elif" => is_field(p, "condition", node),
            _ => false,
        })
}

/// Whether `node` is the member name of `x.Name`, `x?.Name` or `p->Name`,
/// or of `x.Name<T>`, which keeps its text. `ancestors` end with its
/// parent.
fn is_member_name(node: Node<'_>, ancestors: &[Node<'_>]) -> bool {
    let is_name_of = |child: Node<'_>, parent: Option<&Node<'_>>| {
        parent.is_some_and(|p| {
            matches!(
                p.kind(),
                "member_access_expression" | "member_binding_expression"
            ) && is_field(*p, "name", child)
        })
    };
    let mut up = ancestors.iter().rev();
    match up.next() {
        Some(p) if p.kind() == "generic_name" => is_name_of(*p, up.next()),
        parent => is_name_of(node, parent),
    }
}

/// Whether `node` is the `field` child of `parent`.
fn is_field(parent: Node<'_>, field: &str, node: Node<'_>) -> bool {
    parent
        .child_by_field_name(field)
        .is_some_and(|f| f.id() == node.id())
}

/// Whether `node` has a child (named or not) of one of `kinds`.
fn has_child(node: Node<'_>, kinds: &[&str]) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|c| kinds.contains(&c.kind()))
}

/// Whether the node adds one to the enclosing unit's complexity.
fn is_decision_point(node: Node<'_>, parent: Option<Node<'_>>) -> bool {
    let kind = node.kind();
    if DECISION_KINDS.contains(&kind) {
        return true;
    }
    match kind {
        "binary_expression" => operator_in(node, &["&&", "||", "??"]),
        "assignment_expression" => operator_in(node, &["??="]),
        // Each `case` label of a switch statement; `default` does not
        // count, as `else` does not.
        "case" => !node.is_named() && parent.is_some_and(|p| p.kind() == "switch_section"),
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
                // explicit implementations keep their interface
                ("Impl.I.P.get", 42),
                ("Impl.J.P.get", 43),
                ("Impl.I.this[int].get", 44),
                ("Impl.I.Ev.add", 45),
                ("Impl.I.Ev.remove", 45),
                ("Impl.IAdd<Impl>.operator +(Impl, Impl)", 46),
                ("Impl.operator +(Impl, Impl)", 47),
                ("Impl.operator checked +(Impl, Impl)", 48),
                // a generic type keeps its type parameters
                ("Result.Ok()", 51),
                ("Result<T>.Ok()", 52),
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
                // so does an anonymous method
                ("D.Anonymous(List<int>)", 2),
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
                // the `if` counts; `&&` and `||` in `#if` and `#elif` do not
                ("D.Directives(bool)", 2),
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
        line.split(' ').map(str::to_owned).collect()
    }

    // Requirement: C# normalized tokens, "Renamed copy".
    #[test]
    fn a_renamed_copy_has_the_same_tokens() {
        let a = "class A { string Fmt(DateTime d) { var y = d.Year; return $\"{y}-\"; } }";
        let b = "class B { string FormatOrderDate(DateTime when) { var year = when.Year; return $\"{year}/\"; } }";
        assert_eq!(tokens(a), tokens(b));
        assert_eq!(
            tokens(a),
            texts(
                "class $id { string $id ( $id $id ) { var $id = $id . Year return $ \" { $id } $lit \" } }"
            )
        );
    }

    // Requirement: C# normalized tokens, "Different members".
    #[test]
    fn member_names_keep_their_text() {
        let year = tokens("class A { int F(D d) => d.Year + d?.Year + p->Year; }");
        let month = tokens("class A { int F(D d) => d.Month + d?.Month + p->Month; }");
        assert_ne!(year, month);
        assert_eq!(
            year,
            texts("class $id { int $id ( $id $id ) => $id . Year + $id ? . Year + $id -> Year }")
        );
    }

    #[test]
    fn each_literal_is_one_placeholder() {
        let source = "class A { object F() => new object[] { \"e\\t\", @\"v\", 'c', \"\"\"raw\"\"\", 1.5m, 0x1F, true, null }; }";
        assert_eq!(
            tokens(source),
            texts(
                "class $id { object $id ( ) => new object [ ] { $lit , $lit , $lit , $lit , $lit , $lit , true , null } }"
            )
        );
    }

    #[test]
    fn interpolated_strings_keep_their_shape() {
        // A run of text and escapes is one literal; a format is one too.
        assert_eq!(
            tokens("class A { string F(int y) => $\"{y}-\\n{y:D2}x\"; }"),
            texts("class $id { string $id ( int $id ) => $ \" { $id } $lit { $id $lit } $lit \" }")
        );
    }

    // Requirement: Conditional compilation.
    #[test]
    fn code_in_directive_branches_has_tokens() {
        let source = "class C { void F(bool a) {\n#if DEBUG && TRACE\n if (a) { Log(a); }\n\
                      #elif X\n Log(1);\n#else\n Log(2);\n#endif\n} }";
        assert_eq!(
            tokens(source),
            texts(
                "class $id { void $id ( bool $id ) { if ( $id ) { $id ( $id ) } $id ( $lit ) $id ( $lit ) } }"
            )
        );
        assert_eq!(
            tokens("#if NET8\nclass A { }\n#endif\n"),
            texts("class $id { }")
        );
    }

    // Requirement: C# normalized tokens: a generic member keeps its name.
    #[test]
    fn generic_member_names_keep_their_text() {
        let cast = tokens("class A { object F(X x) => x.Cast<int>() ?? x?.OfType<int>(); }");
        let filter = tokens("class A { object F(X x) => x.Where<int>() ?? x?.Select<int>(); }");
        assert_ne!(cast, filter);
        assert_eq!(
            cast,
            texts(
                "class $id { object $id ( $id $id ) => $id . Cast < int > ( ) ?? $id ? . OfType < int > ( ) }"
            )
        );
    }

    // Requirement: C# code without tokens.
    #[test]
    fn usings_namespaces_preprocessor_and_comments_have_no_tokens() {
        let source = "using System;\nusing static System.Math;\nglobal using Alias = System.Text;\n\
                      namespace A.B;\n#nullable enable\n#region R\n/// <summary>doc</summary>\n\
                      // line\n/* block */\nclass T { }\n#endregion\nnamespace C { class U { } }\n";
        assert_eq!(tokens(source), texts("class $id { } { class $id { } }"));
        assert_eq!(
            tokens("using System;\nusing System.Linq;\nnamespace Shop.Orders;\n"),
            Vec::<String>::new()
        );
    }

    // Requirement: C# files, "A file with a byte order mark".
    #[test]
    fn a_byte_order_mark_is_ignored() {
        let source = "\u{feff}class A { int F(bool a) { if (a) { } return 0; } }";
        let model = parse(source).expect("parses");
        assert_eq!(model.functions[0].cyclomatic, 2);
        assert_eq!(model.tokens[0].text, "class");
    }

    #[test]
    fn tokens_carry_their_line() {
        let model = parse("class A\n{\n    int F() => 1;\n}\n").expect("parses");
        let lines: Vec<u32> = model.tokens.iter().map(|t| t.line).collect();
        assert_eq!(lines, [1, 1, 2, 3, 3, 3, 3, 3, 3, 4]);
    }

    // Requirement: Conditional compilation, "Branches that open blocks".
    #[test]
    fn branches_that_open_blocks_are_a_syntax_error() {
        // The unbalanced block puts the whole namespace in one error node,
        // so the first error is on its line.
        assert_eq!(
            parse(include_str!("../fixtures/csharp/syntax_error.cs")),
            Err(ParseError::Syntax { line: 3 })
        );
        assert_eq!(
            parse("class A {\n  void F() {\n    if (x) {\n  }\n}\n"),
            Err(ParseError::Syntax { line: 1 })
        );
        assert_eq!(
            parse("class A {\n  void F() { x = ; }\n}\n"),
            Err(ParseError::Syntax { line: 2 })
        );
    }

    #[test]
    fn declarations_without_a_body() {
        let source = "interface I { void Open(); int Close(int c); }\n\
                      abstract class B { public abstract void Run(); partial void On(); static extern int N(); }";
        assert_eq!(names(source), pairs(&[]));
    }
}
