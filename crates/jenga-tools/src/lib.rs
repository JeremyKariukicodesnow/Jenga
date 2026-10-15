//! Deterministic automation interfaces. No model provider, chat, or inference dependency.
pub mod catalog;
pub mod engine;
pub mod formatting;
pub mod help;
pub mod journal;
pub mod net_guard;
pub mod project_workspace;
pub mod projects;
pub mod protocol;
pub mod render;
pub mod router;
pub mod routing;
pub mod setup;
pub mod symbol_editor;
pub mod transport;

pub mod library_manage;
pub mod operations;
pub mod workflows;

pub mod replacement;
