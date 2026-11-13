//! Language adapters.
//!
//! Each adapter turns the source text of one file into a language-neutral
//! [`FileModel`] that analyzers work on, so analyzers never see syntax
//! trees. Parsing is built on tree-sitter; see
//! `docs/adr/0001-rust-and-tree-sitter.md`. This crate does no I/O: callers
//! read files and pick the [`Language`].
//!
//! Supported: TypeScript, TSX and C#. The model holds one [`Function`] per
//! named function unit with its qualified name, line and cyclomatic
//! complexity. Anonymous functions count towards their nearest named
//! ancestor; code outside named functions is not measured. At module level
//! and in class field initializers a call does not hide the binding: in
//! `export const GET = withAuth(async () => {...})` the functions passed
//! through the call form one unit named `GET`.
//!
//! The model also lists the file's [`Import`]s: the module specifier of
//! every `import` and `export ... from` declaration, `import x =
//! require(...)`, and `import(...)` or `require(...)` call with a string
//! literal, as written and with its line. Resolving specifiers is up to
//! the analyzers.
//!
//! Finally, the model holds the file's code as normalized [`Token`]s for
//! finding copies: identifiers become `$id` and literals `$lit`, while
//! member names, keywords and punctuation keep their text; comments,
//! semicolons, imports and type-level code produce none.
//!
//! ```
//! use qgate_lang::{Language, parse};
//!
//! let model = parse(Language::TypeScript, "function f(a, b) { return a && b; }")?;
//! assert_eq!(model.functions[0].name, "f");
//! assert_eq!(model.functions[0].cyclomatic, 2);
//! # Ok::<(), qgate_lang::ParseError>(())
//! ```

mod common;
mod csharp;
mod generated;
mod model;
mod typescript;

pub use generated::{csharp_generated_header, csharp_generated_name};
pub use model::{FileModel, Function, Import, ParseError, Token};

/// Parse `source`, written in `language`, into a [`FileModel`].
///
/// # Errors
///
/// Returns [`ParseError::Syntax`] if the source contains a syntax error,
/// and [`ParseError::Parser`] if the parser itself fails.
pub fn parse(language: Language, source: &str) -> Result<FileModel, ParseError> {
    match language {
        Language::TypeScript | Language::Tsx => typescript::parse(language, source),
        Language::CSharp => csharp::parse(source),
    }
}

/// A language qgate knows how to analyze.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// TypeScript (`.ts`, `.mts`, `.cts`).
    TypeScript,
    /// TypeScript with JSX (`.tsx`). It needs its own grammar: in TSX,
    /// `<T>x` is a JSX element, in plain TypeScript it is a type assertion.
    Tsx,
    /// C# (`.cs`).
    CSharp,
}

/// Languages whose code can be compared with each other: unit names can
/// match, and copies can be found, only within one family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    /// TypeScript and TSX (and JavaScript, which qgate reads as TSX).
    TypeScript,
    /// C#.
    CSharp,
}

impl Language {
    /// Guess the language from a file extension (without the dot).
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "ts" | "mts" | "cts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "cs" => Some(Self::CSharp),
            _ => None,
        }
    }

    /// The family whose code this language's code can be compared with.
    #[must_use]
    pub fn family(self) -> Family {
        match self {
            Self::TypeScript | Self::Tsx => Family::TypeScript,
            Self::CSharp => Family::CSharp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_typescript() {
        for ext in ["ts", "mts", "cts"] {
            assert_eq!(Language::from_extension(ext), Some(Language::TypeScript));
        }
    }

    #[test]
    fn recognizes_tsx() {
        assert_eq!(Language::from_extension("tsx"), Some(Language::Tsx));
    }

    #[test]
    fn both_grammars_load() {
        for language in [Language::TypeScript, Language::Tsx] {
            assert_eq!(parse(language, ""), Ok(FileModel::default()));
        }
    }

    #[test]
    fn recognizes_csharp() {
        assert_eq!(Language::from_extension("cs"), Some(Language::CSharp));
        assert_eq!(Language::from_extension("csx"), None);
    }

    #[test]
    fn families() {
        assert_eq!(Language::TypeScript.family(), Family::TypeScript);
        assert_eq!(Language::Tsx.family(), Family::TypeScript);
        assert_eq!(Language::CSharp.family(), Family::CSharp);
    }

    #[test]
    fn the_csharp_grammar_loads() {
        assert_eq!(parse(Language::CSharp, ""), Ok(FileModel::default()));
    }

    #[test]
    fn unknown_extension() {
        assert_eq!(Language::from_extension("cobol"), None);
    }
}
