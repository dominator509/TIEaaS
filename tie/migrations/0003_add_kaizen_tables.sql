BEGIN;

CREATE TABLE IF NOT EXISTS kaizen_events (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    request_id TEXT NOT NULL,
    category TEXT NOT NULL,
    severity TEXT NOT NULL,
    component TEXT NOT NULL,
    message TEXT NOT NULL,
    metadata_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_kaizen_events_created_at
ON kaizen_events(created_at DESC);

CREATE TABLE IF NOT EXISTS kaizen_proposals (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    status TEXT NOT NULL,
    source_event_ids_json TEXT NOT NULL,
    title TEXT NOT NULL,
    rationale TEXT NOT NULL,
    diff_json TEXT NOT NULL,
    reviewer TEXT,
    reviewed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_kaizen_proposals_status
ON kaizen_proposals(status);

COMMIT;
