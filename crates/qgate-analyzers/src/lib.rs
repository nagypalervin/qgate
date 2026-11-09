//! Analyzers compare a base and a head revision and emit findings.
//!
//! Planned analyzers:
//!
//! - `architecture`: forbidden dependencies between layers (M2)
//! - `complexity`: per-function complexity increase (M1)
//! - `duplication`: near-duplicate blocks introduced by a change (M2)
//! - `tests`: changed code without changed tests, coverage drop (M3)
//!
//! Status: placeholder.

pub mod complexity;

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
