//! TypeScript and TSX adapter, built on `tree-sitter-typescript`.
//!
//! One walk over the syntax tree finds the function units, gives each a
//! qualified name and counts its decision points (design D3 of the
//! `add-cyclomatic-complexity-check` change). Calls bound to a name at
//! module level or in a class field initializer are handled in the same
//! walk (design of `analyze-wrapped-functions`), and so are the file's
//! imports (design D4 of `add-forbidden-imports`). What counts as a
//! decision point follows the `complexity` rule of `ESLint`.

use std::collections::HashSet;

use tree_sitter::{Node, Parser};

use crate::{FileModel, Function, Import, Language, ParseError};

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

    if let Some(line) = first_error_line(tree.root_node()) {
        return Err(ParseError::Syntax { line });
    }

    let mut walker = Walker::new(source.as_bytes());
    walker.walk(tree.root_node());
    let (functions, imports) = walker.finish();
    Ok(FileModel { functions, imports })
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
/// Imports in them (`import("./x").T`) are still recorded, by a separate
/// scan of the skipped subtree.
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

/// Expressions that wrap a value without changing what it is bound to:
/// `(f)`, `f as T`, `f satisfies T`, `f!`.
const WRAPPER_KINDS: &[&str] = &[
    "parenthesized_expression",
    "as_expression",
    "satisfies_expression",
    "non_null_expression",
];

/// Expressions that can be bound to a name and pass a function, object or
/// class through to that binding (see [`pass_through_top`]).
const CALL_KINDS: &[&str] = &["call_expression", "new_expression", "await_expression"];

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
    bound: bool,
}

/// A call, `new` or `await` bound to a name at module level or directly in
/// a class body (a field initializer), as in
/// `export const GET = withAuth(async () => {...})`. Values that pass
/// through it are named after the binding.
struct Bound {
    /// Id of the topmost node of the call's pass-through chain, so a value
    /// passes through this call when its own chain ends at the same node.
    top: usize,
    /// The binding's name, made unique.
    segment: String,
    /// The unit of the functions that pass through, if there is one.
    unit: Option<usize>,
}

/// What collects the decision points of the code inside a node.
#[derive(Clone, Copy)]
enum Scope {
    /// The unit `functions[i]`.
    Unit(usize),
    /// A class body: its non-function code (field initializers) is not
    /// measured, but a call bound to a field passes its name through.
    ClassBody,
    /// An anonymous function with no unit to count towards, or a static
    /// block: not measured, and calls in it hide their binding as before.
    Opaque,
}

struct Walker<'s> {
    source: &'s [u8],
    /// Names of the named ancestors, joined with `.` for qualified names.
    segments: Vec<String>,
    /// Innermost scope last; empty at module level.
    scopes: Vec<Scope>,
    /// Units in source order (by start position).
    functions: Vec<Function>,
    /// Qualified names already given to a unit.
    taken: HashSet<String>,
    /// Bound calls being walked, innermost last.
    bound: Vec<Bound>,
    /// Imports in source order.
    imports: Vec<Import>,
}

impl<'s> Walker<'s> {
    fn new(source: &'s [u8]) -> Self {
        Self {
            source,
            segments: Vec::new(),
            scopes: Vec::new(),
            functions: Vec::new(),
            taken: HashSet::new(),
            bound: Vec::new(),
            imports: Vec::new(),
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
            if TYPE_KINDS.contains(&node.kind()) {
                // Not entered, but `import("./x").T` in a type still
                // imports a module.
                self.record_imports_below(node);
            } else if cursor.goto_first_child() {
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
        self.record_import(node);

        // A value passed through a bound call takes the binding's name.
        if FUNCTION_KINDS.contains(&kind) || CONTAINER_KINDS.contains(&kind) {
            if let Some(bound) = self
                .bound
                .last()
                .filter(|b| b.top == pass_through_top(node).id())
            {
                self.segments.push(bound.segment.clone());
                frame.segment = true;
                if let (true, Some(unit)) = (FUNCTION_KINDS.contains(&kind), bound.unit) {
                    self.scopes.push(Scope::Unit(unit));
                    frame.scope = true;
                }
                return frame;
            }
        }

        if FUNCTION_KINDS.contains(&kind) {
            // Anonymous functions open no unit: their decision points go to
            // the enclosing unit. Without one, they are opaque.
            if let Some((name, line)) = self.name_of(node) {
                let segment = self.unique_segment(&name);
                self.segments.push(segment);
                let qualified = self.segments.join(".");
                self.taken.insert(qualified.clone());
                self.functions.push(Function {
                    name: qualified,
                    line,
                    cyclomatic: 1,
                });
                self.scopes.push(Scope::Unit(self.functions.len() - 1));
                frame.segment = true;
                frame.scope = true;
            } else if !self.collecting() {
                self.scopes.push(Scope::Opaque);
                frame.scope = true;
            }
        } else if CONTAINER_KINDS.contains(&kind) {
            if let Some((name, _)) = self.name_of(node) {
                self.segments.push(name);
                frame.segment = true;
            }
        } else if kind == "class_body" {
            self.scopes.push(Scope::ClassBody);
            frame.scope = true;
        } else if kind == "class_static_block" {
            self.scopes.push(Scope::Opaque);
            frame.scope = true;
        } else if CALL_KINDS.contains(&kind)
            && matches!(self.scopes.last(), None | Some(Scope::ClassBody))
        {
            // Module level or a class field initializer only.
            frame.bound = self.enter_bound_call(node);
        } else if is_decision_point(node) {
            if let Some(Scope::Unit(index)) = self.scopes.last() {
                self.functions[*index].cyclomatic += 1;
            }
        }
        frame
    }

    /// Whether a unit collects the decision points at the current position.
    fn collecting(&self) -> bool {
        matches!(self.scopes.last(), Some(Scope::Unit(_)))
    }

    /// Start a bound call if `node` is bound to a name. The unit of the
    /// functions passing through is created here, before anything nested
    /// in the call, so units stay in source order; it is not created when
    /// no function passes through.
    fn enter_bound_call(&mut self, node: Node<'_>) -> bool {
        let Some((name, line)) = self.binding_name(node) else {
            return false;
        };
        // Like a container, a call without a unit only adds a segment; a
        // suffix is needed only for a unit whose name is taken.
        let passes = passes_a_function(node);
        let segment = if passes {
            self.unique_segment(&name)
        } else {
            name
        };
        let unit = passes.then(|| {
            let mut parts = self.segments.clone();
            parts.push(segment.clone());
            let qualified = parts.join(".");
            self.taken.insert(qualified.clone());
            self.functions.push(Function {
                name: qualified,
                line,
                cyclomatic: 1,
            });
            self.functions.len() - 1
        });
        self.bound.push(Bound {
            top: pass_through_top(node).id(),
            segment,
            unit,
        });
        true
    }

    /// `name`, or `name#2`, `name#3`, ... if the qualified name it would
    /// produce is already taken. Nested units inherit the suffix because it
    /// is part of the segment.
    fn unique_segment(&self, name: &str) -> String {
        let qualify = |segment: &str| {
            let mut parts = self.segments.clone();
            parts.push(segment.to_owned());
            parts.join(".")
        };
        let mut segment = name.to_owned();
        let mut n = 2;
        while self.taken.contains(&qualify(&segment)) {
            segment = format!("{name}#{n}");
            n += 1;
        }
        segment
    }

    fn leave(&mut self, frame: &Frame) {
        if frame.segment {
            self.segments.pop();
        }
        if frame.scope {
            self.scopes.pop();
        }
        if frame.bound {
            self.bound.pop();
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
    /// the value of, or `default` for an unnamed default export. Type
    /// wrappers in between (`as`, `satisfies`, parentheses, `!`) are looked
    /// through.
    fn binding_name(&self, node: Node<'_>) -> Option<(String, u32)> {
        let mut value = node;
        let mut parent = node.parent()?;
        while WRAPPER_KINDS.contains(&parent.kind()) {
            value = parent;
            parent = parent.parent()?;
        }
        let is_value = parent
            .child_by_field_name("value")
            .is_some_and(|v| v.id() == value.id());
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

    /// Property names as written, except that the outer quotes of a
    /// string key are dropped. Escapes stay as written; computed names keep
    /// their brackets.
    fn property_name(&self, name: Node<'_>) -> String {
        let text = self.text(name);
        if name.kind() == "string" {
            unquote(&text).to_owned()
        } else {
            text
        }
    }

    fn text(&self, node: Node<'_>) -> String {
        node.utf8_text(self.source).unwrap_or_default().to_owned()
    }

    fn finish(self) -> (Vec<Function>, Vec<Import>) {
        (self.functions, self.imports)
    }

    /// Record the import `node` makes, if it is one (design D4 of
    /// `add-forbidden-imports`).
    fn record_import(&mut self, node: Node<'_>) {
        if let Some(source) = import_source(node, self.source) {
            self.imports.push(Import {
                specifier: unquote(&self.text(source)).to_owned(),
                line: line_of(source),
            });
        }
    }

    /// Record the imports in the subtree below `node`, in source order,
    /// for a subtree the walk does not enter.
    fn record_imports_below(&mut self, node: Node<'_>) {
        let mut pending = named_children(node);
        pending.reverse();
        while let Some(next) = pending.pop() {
            self.record_import(next);
            let mut children = named_children(next);
            children.reverse();
            pending.extend(children);
        }
    }
}

/// The topmost node reached from `node` by passing through: type wrappers,
/// `await`, call and `new` arguments, and the function of a call (an
/// immediately invoked function or a curried call). A value passes through
/// a bound call when both chains end at the same node.
fn pass_through_top(node: Node<'_>) -> Node<'_> {
    let mut top = node;
    while let Some(parent) = top.parent() {
        let kind = parent.kind();
        top = if WRAPPER_KINDS.contains(&kind) || kind == "await_expression" {
            parent
        } else if kind == "arguments" {
            match parent.parent() {
                Some(call) => call,
                None => break,
            }
        } else if kind == "call_expression"
            && parent
                .child_by_field_name("function")
                .is_some_and(|f| f.id() == top.id())
        {
            parent
        } else {
            break;
        };
    }
    top
}

/// Whether a function passes through `call`: a bounded scan along the
/// pass-through edges of [`pass_through_top`], downwards. It never enters a
/// function body.
fn passes_a_function(call: Node<'_>) -> bool {
    let mut pending = vec![call];
    while let Some(node) = pending.pop() {
        let kind = node.kind();
        let mut next = Vec::new();
        if WRAPPER_KINDS.contains(&kind) || kind == "await_expression" {
            next.extend(named_children(node));
        } else if kind == "call_expression" || kind == "new_expression" {
            next.extend(node.child_by_field_name("function"));
            if let Some(arguments) = node
                .child_by_field_name("arguments")
                .filter(|a| a.kind() == "arguments")
            {
                next.extend(named_children(arguments));
            }
        }
        for child in next {
            if FUNCTION_KINDS.contains(&child.kind()) {
                return true;
            }
            pending.push(child);
        }
    }
    false
}

/// The string literal naming the module that `node` imports, if `node` is
/// an import: an `import` or `export ... from` declaration, the
/// `require(...)` of `import x = require(...)`, a call of `import(...)`
/// whose first argument is a string literal, or a call of `require(...)`
/// whose only argument is one.
fn import_source<'t>(node: Node<'t>, source: &[u8]) -> Option<Node<'t>> {
    match node.kind() {
        // `import x = require("m")` has its source on the require clause,
        // which the walk visits as a node of its own.
        "import_statement" | "export_statement" | "import_require_clause" => {
            node.child_by_field_name("source")
        }
        "call_expression" => {
            let function = node.child_by_field_name("function")?;
            let is_import = function.kind() == "import";
            let is_require = function.kind() == "identifier"
                && function.utf8_text(source).is_ok_and(|t| t == "require");
            if !is_import && !is_require {
                return None;
            }
            let arguments = node
                .child_by_field_name("arguments")
                .filter(|a| a.kind() == "arguments")?;
            let mut values = named_children(arguments)
                .into_iter()
                .filter(|n| n.kind() != "comment");
            match (values.next(), values.next()) {
                // `import()` may take import attributes as a second
                // argument; `require` takes exactly one.
                (Some(first), rest)
                    if first.kind() == "string" && (is_import || rest.is_none()) =>
                {
                    Some(first)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// A string literal's text without its outer quotes, as written: a string
/// node always starts and ends with its one-byte quote. Escapes stay.
fn unquote(text: &str) -> &str {
    text.get(1..text.len().saturating_sub(1))
        .unwrap_or_default()
}

/// The named children of a node.
fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
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

    /// Parse a fixture and return `(name, line, cyclomatic)` triples.
    fn units(language: Language, source: &str) -> Vec<(String, u32, u32)> {
        parse(language, source)
            .expect("fixture parses")
            .functions
            .into_iter()
            .map(|f| (f.name, f.line, f.cyclomatic))
            .collect()
    }

    fn triples(items: &[(&str, u32, u32)]) -> Vec<(String, u32, u32)> {
        items
            .iter()
            .map(|&(n, l, c)| (n.to_owned(), l, c))
            .collect()
    }

    #[test]
    fn naming_and_unit_boundaries() {
        let actual = units(
            Language::TypeScript,
            include_str!("../fixtures/typescript/naming.ts"),
        );
        assert_eq!(
            actual,
            triples(&[
                ("Cart.constructor", 6, 1),
                // the ?? sits in an anonymous callback: counts towards total
                ("Cart.total", 8, 2),
                ("Cart.handle", 12, 2),
                ("Cart.get size", 18, 1),
                ("Cart.set size", 22, 2),
                // static and instance method with the same name
                ("Cart.create", 28, 1),
                ("Cart.create#2", 32, 1),
                ("Cart.[Symbol.iterator]", 34, 1),
                ("Cart.#secret", 36, 1),
                // `count = this.items?.length ?? 0` is a field initializer:
                // not measured
                ("api.load", 42, 1),
                ("api.save", 43, 2),
                ("api.load-data", 46, 1),
                ("api.nested.deep", 48, 1),
                ("outer", 52, 2),
                ("outer.inner", 56, 3),
                ("process", 66, 2),
                // named after the binding, not `ownName`
                ("bound", 70, 2),
                // a named function expression passed as an argument
                ("callbackWithName", 74, 2),
                // the field initializer `flag ?? false` does not leak
                // into factory
                ("factory", 80, 1),
                ("factory.Widget.render", 83, 2),
                // overload signatures are not units
                ("parse", 91, 2),
                // abstract and overload signatures are not units
                ("Reader.read", 102, 2),
                // describe/it callbacks and the top-level if: not measured
                ("default", 121, 2),
            ])
        );
    }

    /// Shorthand for a TypeScript fixture's `(name, line, cyclomatic)`.
    fn ts_units(source: &str) -> Vec<(String, u32, u32)> {
        units(Language::TypeScript, source)
    }

    #[test]
    fn generators_are_units() {
        assert_eq!(
            ts_units(include_str!("../fixtures/typescript/generators.ts")),
            triples(&[("gen", 3, 2), ("g", 9, 1), ("C.items", 12, 1)])
        );
    }

    #[test]
    fn type_wrappers_do_not_hide_the_binding() {
        assert_eq!(
            ts_units(include_str!("../fixtures/typescript/wrappers.ts")),
            triples(&[
                ("config.load", 4, 1),
                ("other.load", 8, 1),
                ("h", 11, 2),
                ("nonNull", 17, 1),
                ("Widget.render", 20, 1),
            ])
        );
    }

    #[test]
    fn code_without_named_ancestor_is_not_measured() {
        assert_eq!(
            ts_units(include_str!("../fixtures/typescript/module_level.ts")),
            triples(&[])
        );
    }

    #[test]
    fn calls_at_module_level_do_not_hide_the_binding() {
        assert_eq!(
            ts_units(include_str!("../fixtures/typescript/wrapped_calls.ts")),
            triples(&[
                ("GET", 3, 2),
                // nested calls, a member call, type arguments
                ("Button", 10, 2),
                // curried call
                ("Comp", 19, 2),
                ("p", 25, 2),
                ("data", 31, 2),
                // immediately invoked function
                ("config", 38, 2),
                // type wrappers around the call and the function
                ("typed", 46, 2),
                ("default", 52, 2),
            ])
        );
    }

    #[test]
    fn a_bound_call_is_one_unit() {
        assert_eq!(
            ts_units(include_str!("../fixtures/typescript/bound_call_units.ts")),
            triples(&[
                // three functions summed: 1 + ?? + ternary
                ("sel", 4, 3),
                // the ?? in the options object does not count
                ("h", 11, 2),
                // the binding wins over `onDone`
                ("named", 21, 2),
                // objects and classes: prefix only, no unit of their own
                ("useCart.actions.load", 30, 2),
                ("Mixed.m", 39, 2),
                ("X", 47, 2),
                ("X.m", 53, 1),
                // a bound call inside an object that passes through
                ("nested.a", 58, 2),
                ("A.onScroll", 67, 2),
                ("api.load", 75, 2),
                // `plain = compute(a ?? b)` passes no function: no unit
                ("Dup.h", 88, 2),
                // a duplicate gets a suffix; `n ?? 1` does not count
                ("Dup.h#2", 93, 1),
                ("Shadow.create", 99, 1),
                // no unit for the call, so no suffix for its segment
                ("Shadow.create.run", 100, 1),
                // nested names take the prefix; the ternary in the
                // anonymous map callback counts towards Panel
                ("Panel", 105, 2),
                ("Panel.handleClick", 106, 2),
                ("Panel.inner", 111, 1),
                ("Panel.o.m", 112, 1),
                // class fields in a test callback and in a static block
                ("Fake.h", 120, 2),
                ("Outer.Inner.h", 131, 1),
            ])
        );
        assert_eq!(
            ts_units(include_str!(
                "../fixtures/typescript/default_wrapped_object.ts"
            )),
            triples(&[("default.setup", 3, 1)])
        );
    }

    #[test]
    fn inside_a_function_calls_change_nothing() {
        assert_eq!(
            ts_units(include_str!(
                "../fixtures/typescript/wrapped_inside_function.ts"
            )),
            triples(&[
                // the useCallback and reduce callbacks count towards
                // their function
                ("C", 3, 2),
                ("outer", 12, 2),
                // a class field initializer collects nothing, so the rule
                // applies there
                ("make", 19, 1),
                ("make.h", 21, 2),
            ])
        );
    }

    #[test]
    fn field_initializers_and_static_blocks_are_barriers() {
        assert_eq!(
            ts_units(include_str!("../fixtures/typescript/static_block.ts")),
            triples(&[("make", 3, 1)])
        );
    }

    #[test]
    fn unnamed_default_exported_object_and_class() {
        assert_eq!(
            ts_units(include_str!("../fixtures/typescript/default_object.ts")),
            triples(&[("default.load", 2, 2)])
        );
        assert_eq!(
            ts_units(include_str!("../fixtures/typescript/default_class.ts")),
            triples(&[("default.m", 2, 1)])
        );
    }

    #[test]
    fn duplicate_names_are_unique_and_propagate() {
        assert_eq!(
            ts_units(include_str!("../fixtures/typescript/duplicates.ts")),
            triples(&[
                ("o.x#2", 4, 1),
                ("o.x", 5, 1),
                // #2 is taken by the literal key above
                ("o.x#3", 6, 1),
                ("A.f", 10, 1),
                ("A.f.g", 11, 1),
                ("A.f#2", 13, 1),
                ("A.f#2.g", 14, 1),
            ])
        );
    }

    #[test]
    fn property_names_drop_only_the_outer_quotes() {
        assert_eq!(
            ts_units(include_str!("../fixtures/typescript/property_names.ts")),
            triples(&[
                ("o.load-data", 2, 1),
                ("o.it\\'s", 3, 1),
                ("o.[key]", 4, 1),
                ("o.'quoted'", 5, 1),
            ])
        );
    }

    #[test]
    fn imports_that_count() {
        let model = parse(
            Language::TypeScript,
            include_str!("../fixtures/typescript/imports.ts"),
        )
        .expect("fixture parses");
        let actual: Vec<(&str, u32)> = model
            .imports
            .iter()
            .map(|i| (i.specifier.as_str(), i.line))
            .collect();
        assert_eq!(
            actual,
            [
                ("./money", 3),
                // type-only and side-effect imports count
                ("../ui/types", 4),
                ("./polyfills", 5),
                // re-exports with a source count
                ("../infra/db", 6),
                ("./b", 7),
                // import x = require(...)
                ("fs", 8),
                // the line of the specifier, not of `import {`
                ("./multi", 12),
                // dynamic import and require inside a function; template
                // literals, identifiers and two arguments do not count
                ("../ui/Modal", 19),
                ("lodash", 20),
                // type positions, inside and outside skipped type subtrees
                ("./typed", 30),
                ("./typeof", 31),
                ("./asserted", 32),
                // module level, through the bound-call path
                ("./top", 35),
                // import attributes do not hide the module
                ("./data.json", 36),
                // a comment between arguments is skipped
                ("./commented", 39),
                // an empty specifier is kept as written
                ("", 40),
                // source order inside a skipped subtree and around it
                ("./c1", 43),
                ("./c2", 43),
                ("./m1", 44),
                ("./m2", 44),
            ]
        );
    }

    #[test]
    fn tsx_imports() {
        let model = parse(
            Language::Tsx,
            "import React from \"react\";\nexport const A = () => <div />;\n",
        )
        .expect("parses");
        assert_eq!(
            model.imports,
            [Import {
                specifier: "react".to_owned(),
                line: 1,
            }]
        );
    }

    #[test]
    fn syntax_error_reports_first_error_line() {
        let source = include_str!("../fixtures/typescript/syntax_error.ts");
        assert_eq!(
            parse(Language::TypeScript, source),
            Err(ParseError::Syntax { line: 7 })
        );
    }

    #[test]
    fn tsx_uses_the_tsx_grammar() {
        let source = include_str!("../fixtures/typescript/component.tsx");
        assert_eq!(
            units(Language::Tsx, source),
            triples(&[
                // ternary inside JSX
                ("Greeting", 4, 2),
                // && inside the anonymous map callback
                ("List", 8, 2),
            ])
        );
        assert!(matches!(
            parse(Language::TypeScript, source),
            Err(ParseError::Syntax { .. })
        ));
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
