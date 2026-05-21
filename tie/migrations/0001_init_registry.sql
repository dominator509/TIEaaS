BEGIN;

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
    tags_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_registry_key_version
ON registry_records(namespace, kind, key, version DESC);

COMMIT;
