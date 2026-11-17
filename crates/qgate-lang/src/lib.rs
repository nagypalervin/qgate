//! Language adapters.
//!
//! Each adapter turns the source text of one file into a language-neutral
//! [`FileModel`] that analyzers work on, so analyzers never see syntax
//! trees. Parsing is built on tree-sitter; see
//! `docs/adr/0001-rust-and-tree-sitter.md`. This crate does no I/O: callers
//! read files and pick the [`Language`].
//!
//! Supported: TypeScript, TSX, C# and Rust. The model holds one
//! [`Function`] per named function unit with its qualified name, line,
//! cyclomatic complexity and a fingerprint of its code, which tells
//! whether the unit changed between two versions. Anonymous functions
//! count towards their nearest named ancestor; code outside named
//! functions is not measured.
//! At module level and in class field initializers a call does not hide
//! the binding: in `export const GET = withAuth(async () => {...})` the
//! functions passed through the call form one unit named `GET`.
//!
//! For TypeScript, the model also lists the file's [`Import`]s: the
//! module specifier of every `import` and `export ... from` declaration,
//! `import x = require(...)`, and `import(...)` or `require(...)` call
//! with a string literal, as written and with its line. Resolving
//! specifiers is up to the analyzers. For C#, it lists the file's
//! [`Using`]s instead: the namespace or type each `using` directive names,
//! normalized, with its line.
//!
//! Finally, the model holds the file's code as normalized [`Token`]s for
//! finding copies: identifiers become `$id` and literals `$lit`, while
//! member names, keywords and punctuation keep their text; comments and
//! semicolons produce none, nor do TypeScript's imports and type-level
//! code, or C#'s `using` directives, namespace declarations and
//! preprocessor directives, or Rust's `use` declarations, module
//! declarations and attributes. C# and Rust types are code and keep their
//! tokens; Rust macro calls are compared by their tokens too. Next to the
//! tokens, it holds a hash of each line's trimmed text, so lines that
//! moved can be told from lines that were written.
//!
//! C# units are named by their types and signature, such as
//! `Cart.Total(int)`; [`csharp_generated_name`] and
//! [`csharp_generated_header`] tell generated C# files. Rust units are
//! named by their inline modules and their `impl` or trait, such as
//! `Cart.total` or `<Cart as Display>.fmt`.
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
mod rust;
mod typescript;

pub use generated::{csharp_generated_header, csharp_generated_name};
pub use model::{FileModel, Function, Import, ParseError, Token, Using};

/// Parse `source`, written in `language`, into a [`FileModel`].
///
/// # Errors
///
/// Returns [`ParseError::Syntax`] if the source contains a syntax error,
/// and [`ParseError::Parser`] if the parser itself fails.
pub fn parse(language: Language, source: &str) -> Result<FileModel, ParseError> {
    let mut model = match language {
        Language::TypeScript | Language::Tsx => typescript::parse(language, source),
        Language::CSharp => csharp::parse(source),
        Language::Rust => rust::parse(source),
    }?;
    model.lines = common::line_hashes(source);
    Ok(model)
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
    /// Rust (`.rs`).
    Rust,
}

/// Languages whose code can be compared with each other: unit names can
/// match, and copies can be found, only within one family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    /// TypeScript and TSX (and JavaScript, which qgate reads as TSX).
    TypeScript,
    /// C#.
    CSharp,
    /// Rust.
    Rust,
}

impl Family {
    /// Every family, each once (design D1 of `add-rust-analysis`).
    pub const ALL: [Self; 3] = [Self::TypeScript, Self::CSharp, Self::Rust];
}

impl Language {
    /// Guess the language from a file extension (without the dot).
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "ts" | "mts" | "cts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "cs" => Some(Self::CSharp),
            "rs" => Some(Self::Rust),
            _ => None,
        }
    }

    /// The family whose code this language's code can be compared with.
    #[must_use]
    pub fn family(self) -> Family {
        match self {
            Self::TypeScript | Self::Tsx => Family::TypeScript,
            Self::CSharp => Family::CSharp,
            Self::Rust => Family::Rust,
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
    fn recognizes_rust() {
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
        assert_eq!(Language::from_extension("rlib"), None);
    }

    #[test]
    fn families() {
        assert_eq!(Language::TypeScript.family(), Family::TypeScript);
        assert_eq!(Language::Tsx.family(), Family::TypeScript);
        assert_eq!(Language::CSharp.family(), Family::CSharp);
        assert_eq!(Language::Rust.family(), Family::Rust);
    }

    // Design D1 of `add-rust-analysis`: every family once, so a loop over
    // the families cannot miss one.
    #[test]
    fn all_families() {
        for language in [
            Language::TypeScript,
            Language::Tsx,
            Language::CSharp,
            Language::Rust,
        ] {
            assert!(Family::ALL.contains(&language.family()));
        }
        let unique: std::collections::HashSet<_> = Family::ALL.iter().collect();
        assert_eq!(unique.len(), Family::ALL.len());
    }

    #[test]
    fn the_csharp_grammar_loads() {
        assert_eq!(parse(Language::CSharp, ""), Ok(FileModel::default()));
    }

    #[test]
    fn the_rust_grammar_loads() {
        assert_eq!(parse(Language::Rust, ""), Ok(FileModel::default()));
    }

    #[test]
    fn unknown_extension() {
        assert_eq!(Language::from_extension("cobol"), None);
    }

    /// The line hashes of `source`, parsed as `language`.
    fn lines(language: Language, source: &str) -> Vec<u64> {
        parse(language, source).expect("valid source").lines
    }

    #[test]
    fn each_line_is_hashed_by_its_trimmed_text() {
        for (language, code) in [
            (Language::TypeScript, "let a = 1;"),
            (Language::CSharp, "class A {}"),
            (Language::Rust, "fn a() {}"),
        ] {
            let plain = lines(language, &format!("{code}\n// note\n\n{code}\n"));
            assert_eq!(plain.len(), 4, "{language:?}");
            assert_eq!(plain[0], plain[3], "{language:?}");
            assert_ne!(plain[0], plain[1], "{language:?}");
            // An empty line is 0, also one with white space only.
            assert_eq!(plain[2], 0, "{language:?}");
            assert_ne!(plain[0], 0, "{language:?}");
            // Indentation, trailing white space, CRLF and a missing last
            // newline change nothing.
            let messy = lines(
                language,
                &format!("\t  {code}  \r\n  // note\r\n \t \r\n    {code}"),
            );
            assert_eq!(messy, plain, "{language:?}");
        }
    }

    #[test]
    fn a_line_is_hashed_by_fnv_1a() {
        assert_eq!(lines(Language::TypeScript, "a"), [0xaf63_dc4c_8601_ec8c]);
    }

    #[test]
    fn lines_end_at_newlines_only() {
        // An empty last line counts; a lone carriage return ends no line,
        // as for tree-sitter.
        let hashes = lines(Language::TypeScript, "a\n\n");
        assert_eq!(hashes.len(), 2);
        assert_eq!(hashes[1], 0);
        assert_eq!(lines(Language::TypeScript, "const s = `a\rb`;").len(), 1);
    }

    #[test]
    fn a_byte_order_mark_and_unicode_spaces_are_trimmed() {
        let bom = char::from_u32(0xFEFF).expect("a char");
        let nbsp = char::from_u32(0xA0).expect("a char");
        let plain = lines(Language::CSharp, "class A {}\nclass B {}");
        let marked = lines(
            Language::CSharp,
            &format!("{bom}class A {{}}\n{nbsp}class B {{}}{nbsp}"),
        );
        assert_eq!(marked, plain);
    }

    #[test]
    fn every_token_is_on_a_line_with_code() {
        for (language, source) in [
            (
                Language::TypeScript,
                "// c\r\nconst s = `a\r\nb`;\r\n\r\nf(s);",
            ),
            (
                Language::CSharp,
                "class A {\n  // c\n  string s = @\"a\nb\";\n\n  int F() => 1;\n}",
            ),
            (
                Language::Rust,
                "// c\nfn f() -> &'static str {\n    r\"a\nb\"\n}\n",
            ),
        ] {
            let model = parse(language, source).expect("valid source");
            assert_ne!(model.tokens.len(), 0, "{language:?}");
            for token in &model.tokens {
                let line = usize::try_from(token.line).expect("small") - 1;
                assert_ne!(model.lines[line], 0, "{language:?} {token:?}");
            }
        }
    }

    #[test]
    fn a_file_without_code_has_its_lines() {
        assert_eq!(lines(Language::TypeScript, "// a\n// b\n").len(), 2);
        assert_eq!(lines(Language::TypeScript, ""), Vec::<u64>::new());
    }
}
