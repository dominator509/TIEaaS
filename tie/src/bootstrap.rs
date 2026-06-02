use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Executor, SqlitePool};

/// Minimal report emitted after bootstrap so operators and tests can confirm
/// that the service started against the expected persistence shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootReport {
    pub booted_at: String,
    pub registry_table_present: bool,
    pub policies_table_present: bool,
    pub kaizen_table_present: bool,
}

/// Database bootstrap seam. In the current Phase 0 code, migrations are also
/// embedded in `src/main.rs`; this module provides the extraction target for
/// moving schema ownership out of the binary entrypoint.
#[derive(Debug, Default, Clone, Copy)]
pub struct DatabaseBootstrap;

impl DatabaseBootstrap {
    pub async fn ensure(pool: &SqlitePool) -> Result<BootReport, sqlx::Error> {
        pool.execute(
            r#"
            CREATE TABLE IF NOT EXISTS registry_records (
                id TEXT PRIMARY KEY,
                namespace TEXT NOT NULL,
                kind TEXT NOT NULL,
                key TEXT NOT NULL,
                version INTEGER NOT NULL,
                value_json TEXT NOT NULL,
                provenance_json TEXT NOT NULL,
                digest_sha256 TEXT NOT NULL,
                signature_ed25519 TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                retired_at TEXT,
                tags_json TEXT NOT NULL,
                UNIQUE(namespace, kind, key, version)
            );
            "#,
        )
        .await?;

        pool.execute(
            r#"
            CREATE TABLE IF NOT EXISTS policy_profiles (
                id TEXT PRIMARY KEY,
                profile_name TEXT NOT NULL UNIQUE,
                mode TEXT NOT NULL,
                config_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
        )
        .await?;

        pool.execute(
            r#"
            CREATE TABLE IF NOT EXISTS kaizen_events (
                id TEXT PRIMARY KEY,
                request_id TEXT NOT NULL,
                category TEXT NOT NULL,
                severity TEXT NOT NULL,
                component TEXT NOT NULL,
                message TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .await?;

        Ok(BootReport {
            booted_at: Utc::now().to_rfc3339(),
            registry_table_present: true,
            policies_table_present: true,
            kaizen_table_present: true,
        })
    }
}
