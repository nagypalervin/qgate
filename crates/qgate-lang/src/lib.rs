//! Language adapters.
//!
//! Each adapter turns source files into a language-neutral model (functions,
//! imports, spans) that analyzers work on. Parsing will be built on
//! tree-sitter; see `docs/adr/0001-rust-and-tree-sitter.md`.
//!
//! Status: placeholder. The first adapter (TypeScript) lands in milestone M1.

mod model;
mod typescript;

pub use model::{FileModel, Function, ParseError};

/// Parse `source`, written in `language`, into a [`FileModel`].
///
/// # Errors
///
/// Returns [`ParseError::Syntax`] if the source contains a syntax error,
/// and [`ParseError::Parser`] if the parser itself fails.
pub fn parse(language: Language, source: &str) -> Result<FileModel, ParseError> {
    match language {
        Language::TypeScript | Language::Tsx => typescript::parse(language, source),
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
}

impl Language {
    /// Guess the language from a file extension (without the dot).
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "ts" | "mts" | "cts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            _ => None,
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
    fn unknown_extension() {
        assert_eq!(Language::from_extension("cobol"), None);
    }
}
