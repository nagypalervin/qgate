//! The language-neutral model that adapters produce and analyzers consume.

use std::fmt;

/// Everything qgate extracted from one source file: its function units,
/// its imports and its normalized tokens.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileModel {
    /// Function units in source order. Qualified names are unique within
    /// the file.
    pub functions: Vec<Function>,
    /// Imports in source order, wherever they appear in the file.
    pub imports: Vec<Import>,
    /// The file's code as normalized tokens, in source order, for
    /// comparing code: see [`Token`].
    pub tokens: Vec<Token>,
}

/// One normalized token. Identifiers become `$id` and literals `$lit`;
/// member names, object keys, keywords, operators and punctuation keep
/// their text. Comments, imports, `export ... from` and type-level code
/// produce no tokens, so two copies that differ only in names, values or
/// types have the same tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The normalized text.
    pub text: String,
    /// 1-based line of the token.
    pub line: u32,
}

/// One import of another module: an `import` or `export ... from`
/// declaration, `import x = require(...)`, or a call of `import(...)` or
/// `require(...)` with a string literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    /// The module specifier as written between the quotes, e.g.
    /// `../ui/Button` or `react`. Escapes are not decoded.
    pub specifier: String,
    /// 1-based line of the specifier's string literal.
    pub line: u32,
}

/// One function unit: a named function, method, getter, setter or
/// constructor, including the anonymous functions nested inside it. At
/// module level and in class field initializers it can also be the
/// functions passed through one call to a binding, as in
/// `export const GET = withAuth(async () => {...})`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// Qualified name, unique within the file, e.g. `Cart.total` or
    /// `outer.inner`. Later duplicates get a `#2`, `#3` suffix.
    pub name: String,
    /// 1-based line of the function's name or binding (or of the function
    /// itself, or the call, if it has no name of its own, e.g. an unnamed
    /// default export).
    pub line: u32,
    /// Cyclomatic complexity: 1 plus the number of decision points.
    pub cyclomatic: u32,
}

/// Why a source file could not be turned into a [`FileModel`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The source contains a syntax error, or syntax newer than the
    /// grammar supports. Counting a partially parsed file could silently
    /// distort the results, so it is refused.
    Syntax {
        /// 1-based line of the first syntax error.
        line: u32,
    },
    /// The parser itself failed, e.g. the grammar could not be loaded.
    /// Not caused by the input; indicates a bug or a broken build.
    Parser {
        /// What went wrong.
        message: String,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The grammar can lag behind the language, so valid code with
            // very new syntax lands here too; the message must not claim
            // the user's code is certainly wrong.
            Self::Syntax { line } => write!(
                f,
                "syntax error on line {line} (or syntax the parser does not support yet)"
            ),
            Self::Parser { message } => write!(f, "parser failure: {message}"),
        }
    }
}

impl std::error::Error for ParseError {}
