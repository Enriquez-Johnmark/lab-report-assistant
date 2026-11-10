//! Lab Report Assistant: a patient's lab results become a plain-language
//! report. The model drafts it, code checks it, a physician approves the exact
//! text, the engine delivers the notice once, and every step is audited.

/// The engine, Nudgewright, pinned to a release.
pub use nudgewright as engine;

use std::sync::LazyLock;

use sqlx::migrate::Migrator;

/// The engine's migrations, then this project's, run as one set.
///
/// They share sqlx's migrations table, so two separate migrators would each
/// refuse the other's entries. This project's own migrations join it once
/// the first one exists.
pub static MIGRATOR: LazyLock<&'static Migrator> = LazyLock::new(|| &engine::MIGRATOR);
