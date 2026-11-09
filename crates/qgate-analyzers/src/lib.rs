//! Analyzers compare a base and a head revision and emit findings.
//!
//! Each analyzer implements [`Analyzer`]: it receives a [`FileChange`] (the
//! language-neutral models of one file before and after a change) and
//! returns what got worse. Analyzers never parse, read files or print.
//!
//! Available:
//!
//! - [`complexity`]: per-function cyclomatic complexity increase, and new
//!   functions that are already complex.
//! - [`architecture`]: imports a change introduces against a forbid rule.
//! - [`duplication`]: blocks a change newly duplicates. A repository-level
//!   analysis with its own index rather than an [`Analyzer`].
//!
//! Planned:
//!
//! - `tests`: changed code without changed tests, coverage drop (M3)
//!
//! ```
//! use std::path::PathBuf;
//! use qgate_analyzers::{Analyzer, FileChange, complexity::ComplexityAnalyzer};
//! use qgate_lang::{Language, parse};
//!
//! let change = FileChange {
//!     path: PathBuf::from("src/f.ts"),
//!     old_path: None,
//!     before: parse(Language::TypeScript, "function f(a) { return a; }")?,
//!     after: parse(Language::TypeScript, "function f(a) { return a ? 1 : 0; }")?,
//! };
//! let findings = ComplexityAnalyzer.analyze(&change);
//! assert_eq!(findings[0].message, "f: cyclomatic complexity 1 -> 2 (+1)");
//! # Ok::<(), qgate_lang::ParseError>(())
//! ```

pub mod architecture;
pub mod complexity;
pub mod duplication;

use std::path::PathBuf;

use qgate_core::Finding;
use qgate_lang::FileModel;

/// One file as it was before and after a change.
///
/// Provisional: git input will need added and deleted files and many files
/// per run, and will revisit this type (design D2 of the
/// `add-cyclomatic-complexity-check` change).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// Path of the file after the change, as findings should report it.
    pub path: PathBuf,
    /// Path of the file before the change, if it was renamed; `None` when
    /// the path did not change or the file is new.
    pub old_path: Option<PathBuf>,
    /// The file at the base revision.
    pub before: FileModel,
    /// The file at the head revision.
    pub after: FileModel,
}

/// Something that inspects a change and reports what got worse.
pub trait Analyzer {
    /// Stable name used in config and in rule ids, e.g. `complexity`.
    fn name(&self) -> &'static str;

    /// Compare the two versions of a file and return findings. An empty
    /// list means nothing got worse.
    fn analyze(&self, change: &FileChange) -> Vec<Finding>;
}
