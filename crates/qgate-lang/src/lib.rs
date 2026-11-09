//! Language adapters.
//!
//! Each adapter turns source files into a language-neutral model (functions,
//! imports, spans) that analyzers work on. Parsing will be built on
//! tree-sitter; see `docs/adr/0001-rust-and-tree-sitter.md`.
//!
//! Status: placeholder. The first adapter (TypeScript) lands in milestone M1.

/// A language qgate knows how to analyze.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// TypeScript and TSX.
    TypeScript,
}

impl Language {
    /// Guess the language from a file extension (without the dot).
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "ts" | "tsx" | "mts" | "cts" => Some(Self::TypeScript),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_typescript() {
        assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("tsx"), Some(Language::TypeScript));
    }

    #[test]
    fn unknown_extension() {
        assert_eq!(Language::from_extension("cobol"), None);
    }
}
