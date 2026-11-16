//! What every tree-sitter adapter needs, so each adapter holds only its
//! language's rules (design of `share-adapter-walker`):
//!
//! - building the tree and refusing a file with syntax errors, and lines;
//! - [`walk`] over the tree with a [`Visit`]or, which enters each node and
//!   leaves it again, so state a node opens is closed after its subtree;
//! - [`Units`]: the function units with their names, made unique (design
//!   D1 of `add-csharp-analysis`), the scopes that collect decision
//!   points, and the fingerprints;
//! - [`Tokens`]: the normalized tokens and the muted state;
//! - small node helpers, and types written without whitespace.
//!
//! Which nodes are units, how they are named, what counts as a decision
//! point and which tokens a node gives stay in each adapter.

use std::collections::HashSet;
use std::hash::{DefaultHasher, Hasher};

use tree_sitter::{Language as Grammar, Node, Parser, Tree};

use crate::{FileModel, Function, ParseError, Token};

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
/// Only subtrees that contain an error are visited. A node that has an
/// error but no child with one is the error itself: the C# grammar's
/// missing identifier reports `has_error` but not `is_missing` (task 1.8
/// of `add-csharp-analysis`).
fn first_error_line(root: Node<'_>) -> Option<u32> {
    if !root.has_error() {
        return None;
    }
    let mut first: Option<Node<'_>> = None;
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        let mut cursor = node.walk();
        let inner: Vec<Node<'_>> = node.children(&mut cursor).filter(Node::has_error).collect();
        if node.is_error() || node.is_missing() || inner.is_empty() {
            if first.is_none_or(|f| node.start_byte() < f.start_byte()) {
                first = Some(node);
            }
            continue;
        }
        pending.extend(inner);
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

/// `text` without whitespace, except one space between two words:
/// `Dictionary<string, int>` becomes `Dictionary<string,int>`, and
/// `scoped Span<int>` stays as it is.
pub(crate) fn compact(text: &str) -> String {
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

/// What an adapter does on its way through a syntax tree (design D1 of
/// `share-adapter-walker`): [`walk`] calls `enter` on every node, walks
/// its children if `descend` allows, and then calls `leave` with what
/// `enter` returned, so each node can undo what it set up.
pub(crate) trait Visit {
    /// What entering a node pushed, so leaving it can pop the same.
    type Frame;

    /// Called on entering `node`; `ancestors` end with its parent.
    fn enter(&mut self, node: Node<'_>, ancestors: &[Node<'_>]) -> Self::Frame;

    /// Called after the node and everything below it.
    fn leave(&mut self, frame: Self::Frame);

    /// Whether to walk into the node's children; called after `enter`.
    fn descend(&mut self, _node: Node<'_>) -> bool {
        true
    }
}

/// Parse `source` with `grammar`, walk the tree with `visitor` and turn
/// what it found into the file's model.
pub(crate) fn run<V>(
    grammar: &Grammar,
    source: &str,
    mut visitor: V,
) -> Result<FileModel, ParseError>
where
    V: Visit + Into<FileModel>,
{
    let tree = parse_tree(grammar, source)?;
    walk(tree.root_node(), &mut visitor);
    Ok(visitor.into())
}

/// The source text of `node`.
pub(crate) fn node_text(node: Node<'_>, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or_default().to_owned()
}

/// Walk the tree below `root` depth first, without recursion, so deeply
/// nested code cannot overflow the stack. The ancestors are kept on the
/// way, so no node has to ask tree-sitter for its parent.
pub(crate) fn walk<V: Visit>(root: Node<'_>, visitor: &mut V) {
    let mut cursor = root.walk();
    let mut frames = Vec::new();
    let mut ancestors: Vec<Node<'_>> = Vec::new();
    loop {
        let node = cursor.node();
        frames.push(visitor.enter(node, &ancestors));
        if visitor.descend(node) && cursor.goto_first_child() {
            ancestors.push(node);
            continue;
        }
        loop {
            if let Some(frame) = frames.pop() {
                visitor.leave(frame);
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

/// What a scope tells about the code in it: the index of the unit that
/// collects its decision points and fingerprint, if any.
pub(crate) trait UnitScope: Copy {
    /// The unit the code in this scope belongs to.
    fn unit(self) -> Option<usize>;
}

/// The scope of C# and Rust: a unit, or code that is not measured.
impl UnitScope for Option<usize> {
    fn unit(self) -> Option<usize> {
        self
    }
}

/// How many name segments and scopes were open, so leaving a node can
/// close what entering it opened.
#[derive(Clone, Copy)]
pub(crate) struct Mark {
    segments: usize,
    scopes: usize,
}

/// The function units of a file and what names and measures them while
/// the walk goes (design D2 of `share-adapter-walker`): the name segments
/// of the enclosing items, the names already given, the scopes that
/// collect decision points, and one fingerprint hasher per unit.
pub(crate) struct Units<S> {
    segments: Vec<String>,
    taken: HashSet<String>,
    scopes: Vec<S>,
    functions: Vec<Function>,
    hashers: Vec<DefaultHasher>,
}

impl<S: UnitScope> Units<S> {
    pub(crate) fn new() -> Self {
        Self {
            segments: Vec::new(),
            taken: HashSet::new(),
            scopes: Vec::new(),
            functions: Vec::new(),
            hashers: Vec::new(),
        }
    }

    /// What is open now; [`Self::restore`] closes what opened since.
    pub(crate) fn mark(&self) -> Mark {
        Mark {
            segments: self.segments.len(),
            scopes: self.scopes.len(),
        }
    }

    /// Close the segments and scopes opened since `mark`.
    pub(crate) fn restore(&mut self, mark: Mark) {
        self.segments.truncate(mark.segments);
        self.scopes.truncate(mark.scopes);
    }

    pub(crate) fn push_segment(&mut self, segment: String) {
        self.segments.push(segment);
    }

    pub(crate) fn push_scope(&mut self, scope: S) {
        self.scopes.push(scope);
    }

    /// The innermost scope; `None` at top level.
    pub(crate) fn scope(&self) -> Option<S> {
        self.scopes.last().copied()
    }

    /// The unit the current code belongs to, if any.
    pub(crate) fn unit(&self) -> Option<usize> {
        self.scope().and_then(UnitScope::unit)
    }

    /// `name`, or `name#2`, ... if its qualified name is taken.
    pub(crate) fn unique_segment(&self, name: &str) -> String {
        unique_segment(&self.segments, &self.taken, name)
    }

    /// The qualified name `segment` would have as the last segment.
    pub(crate) fn qualify(&self, segment: &str) -> String {
        let mut parts = self.segments.clone();
        parts.push(segment.to_owned());
        parts.join(".")
    }

    /// Record the unit `qualified` and return its index; it opens no
    /// segment or scope.
    pub(crate) fn add(&mut self, qualified: String, line: u32) -> usize {
        self.taken.insert(qualified.clone());
        self.functions.push(Function {
            name: qualified,
            line,
            cyclomatic: 1,
            fingerprint: 0,
        });
        self.hashers.push(DefaultHasher::new());
        self.functions.len() - 1
    }

    /// Open the unit `name` inside the current segments: push its unique
    /// segment and record it. Return its index; the caller pushes its
    /// scope.
    pub(crate) fn open(&mut self, name: &str, line: u32) -> usize {
        let segment = self.unique_segment(name);
        self.segments.push(segment);
        let qualified = self.segments.join(".");
        self.add(qualified, line)
    }

    /// Add `points` to the complexity of the current unit, if any.
    pub(crate) fn count(&mut self, points: u32) {
        if let Some(unit) = self.unit() {
            self.functions[unit].cyclomatic += points;
        }
    }

    /// The fingerprint hasher of `unit`.
    pub(crate) fn hasher(&mut self, unit: usize) -> &mut DefaultHasher {
        &mut self.hashers[unit]
    }

    /// The units in source order, with their fingerprints.
    pub(crate) fn finish(mut self) -> Vec<Function> {
        for (function, hasher) in self.functions.iter_mut().zip(&self.hashers) {
            function.fingerprint = hasher.finish();
        }
        self.functions
    }
}

/// The normalized tokens of a file (design D4 of `share-adapter-walker`),
/// whether the walk is in a subtree that produces none, and whether the
/// last token was a text part, so the next continues the same literal.
#[derive(Default)]
pub(crate) struct Tokens {
    list: Vec<Token>,
    muted: bool,
    in_text_run: bool,
}

impl Tokens {
    pub(crate) fn muted(&self) -> bool {
        self.muted
    }

    /// Mute the subtree being entered, and note in its frame's `muted`
    /// that leaving it unmutes.
    pub(crate) fn mute(&mut self, muted: &mut bool) {
        self.muted = true;
        *muted = true;
    }

    pub(crate) fn unmute(&mut self) {
        self.muted = false;
    }

    pub(crate) fn in_text_run(&self) -> bool {
        self.in_text_run
    }

    /// Record `text` at the line of `node`; `text_part` says whether it is
    /// a part of a literal's text.
    pub(crate) fn push(&mut self, text: String, node: Node<'_>, text_part: bool) {
        self.list.push(Token {
            text,
            line: line_of(node),
        });
        self.in_text_run = text_part;
    }

    pub(crate) fn finish(self) -> Vec<Token> {
        self.list
    }
}

/// Whether `node` has a child (named or not) of one of `kinds`.
pub(crate) fn has_child(node: Node<'_>, kinds: &[&str]) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|c| kinds.contains(&c.kind()))
}

/// Whether the node's `operator` field is one of `operators`.
pub(crate) fn operator_in(node: Node<'_>, operators: &[&str]) -> bool {
    node.child_by_field_name("operator")
        .is_some_and(|op| operators.contains(&op.kind()))
}
