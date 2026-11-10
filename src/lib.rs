//! Lab Report Assistant: a patient's lab results become a plain-language
//! report. The model drafts it, code checks it, a physician approves the exact
//! text, the engine delivers the notice once, and every step is audited.

pub mod audit;
pub mod checks;
pub mod delivery;
pub mod demo_data;
pub mod drafting;
pub mod drafts;
pub mod evidence;
pub mod fhir;
pub mod html;
pub mod lab_reports;
pub mod made_up;
pub mod model;
pub mod review;
pub mod web;

/// The engine, Nudgewright, pinned to a release.
pub use nudgewright as engine;

use std::borrow::Cow;
use std::sync::LazyLock;

use sqlx::migrate::Migrator;

/// The engine's migrations, then this project's, run as one set.
///
/// They share sqlx's migrations table, so two separate migrators would each
/// refuse the other's entries. sqlx marks `Migrator`'s fields as exempt from
/// semver; re-check this after upgrading sqlx.
pub static MIGRATOR: LazyLock<Migrator> = LazyLock::new(|| {
    static PROJECT: Migrator = sqlx::migrate!("./migrations");
    let mut migrations: Vec<_> = engine::MIGRATOR.migrations.iter().cloned().collect();
    migrations.extend(PROJECT.migrations.iter().cloned());
    migrations.sort_by_key(|migration| migration.version);
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
});
