//! Screen-level views (plan §6): render methods for each screen/region.
//! State and logic stay in `crate::app` (mod.rs); these are extension
//! `impl XWikiApp` blocks, so private fields remain reachable.

pub mod document;
pub mod editor;
pub mod history;
pub mod login;
pub mod settings;
pub mod shell;
pub mod workspace;
