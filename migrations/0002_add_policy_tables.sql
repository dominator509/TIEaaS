BEGIN;

CREATE TABLE IF NOT EXISTS policy_profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    mode TEXT NOT NULL,
    require_fact_citations INTEGER NOT NULL DEFAULT 1,
    require_action_approval INTEGER NOT NULL DEFAULT 1,
    verifier_budget_ms INTEGER NOT NULL,
    config_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS verifier_overrides (
    id TEXT PRIMARY KEY,
    profile_name TEXT NOT NULL,
    verifier_kind TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    timeout_ms INTEGER NOT NULL,
    fallback_mode TEXT NOT NULL,
    config_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_verifier_overrides_profile_kind
ON verifier_overrides(profile_name, verifier_kind);

COMMIT;
