from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional
import json
import sqlite3

from models import ValidationReport


@dataclass(slots=True)
class SQLiteMemoryStore:
    database_path: str = "./zeroclaw-hook/tie_validation_reports.db"

    def __post_init__(self) -> None:
        Path(self.database_path).parent.mkdir(parents=True, exist_ok=True)
        self._init_schema()

    def _connect(self) -> sqlite3.Connection:
        connection = sqlite3.connect(self.database_path)
        connection.row_factory = sqlite3.Row
        return connection

    def _init_schema(self) -> None:
        with self._connect() as conn:
            conn.execute(
                """
                CREATE TABLE IF NOT EXISTS validation_reports (
                    report_id TEXT PRIMARY KEY,
                    created_at TEXT NOT NULL,
                    provider TEXT,
                    model TEXT,
                    conversation_id TEXT,
                    agent_id TEXT,
                    user_id TEXT,
                    blocked INTEGER NOT NULL,
                    block_reason TEXT,
                    released_output TEXT,
                    request_id TEXT NOT NULL,
                    subject_type TEXT NOT NULL,
                    request_payload TEXT NOT NULL,
                    response_payload TEXT NOT NULL,
                    verdict TEXT,
                    score REAL
                )
                """
            )
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_validation_reports_created_at ON validation_reports(created_at DESC)"
            )
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_validation_reports_request_id ON validation_reports(request_id)"
            )
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_validation_reports_verdict ON validation_reports(verdict)"
            )

    def save_report(self, report: ValidationReport) -> None:
        record = report.to_record()
        with self._connect() as conn:
            conn.execute(
                """
                INSERT OR REPLACE INTO validation_reports (
                    report_id, created_at, provider, model, conversation_id, agent_id, user_id,
                    blocked, block_reason, released_output, request_id, subject_type,
                    request_payload, response_payload, verdict, score
                ) VALUES (
                    :report_id, :created_at, :provider, :model, :conversation_id, :agent_id, :user_id,
                    :blocked, :block_reason, :released_output, :request_id, :subject_type,
                    :request_payload, :response_payload, :verdict, :score
                )
                """,
                {
                    **record,
                    "blocked": 1 if record["blocked"] else 0,
                },
            )

    def get_report(self, report_id: str) -> Optional[Dict[str, Any]]:
        with self._connect() as conn:
            row = conn.execute(
                "SELECT * FROM validation_reports WHERE report_id = ?",
                (report_id,),
            ).fetchone()
            return dict(row) if row else None

    def list_recent(self, limit: int = 50) -> List[Dict[str, Any]]:
        with self._connect() as conn:
            rows = conn.execute(
                "SELECT * FROM validation_reports ORDER BY created_at DESC LIMIT ?",
                (limit,),
            ).fetchall()
            return [dict(row) for row in rows]

    def summarize_failures(self, limit: int = 1000) -> Dict[str, Any]:
        rows = self.list_recent(limit=limit)
        verdict_counts: Dict[str, int] = {}
        subject_type_counts: Dict[str, int] = {}
        blocked_count = 0

        for row in rows:
            verdict = row.get("verdict") or "unknown"
            subject_type = row.get("subject_type") or "unknown"
            verdict_counts[verdict] = verdict_counts.get(verdict, 0) + 1
            subject_type_counts[subject_type] = subject_type_counts.get(subject_type, 0) + 1
            blocked_count += int(row.get("blocked") or 0)

        return {
            "sample_size": len(rows),
            "blocked_count": blocked_count,
            "verdict_counts": verdict_counts,
            "subject_type_counts": subject_type_counts,
        }

    def export_jsonl(self, output_path: str, limit: int = 1000) -> str:
        output = Path(output_path)
        output.parent.mkdir(parents=True, exist_ok=True)
        rows = self.list_recent(limit=limit)
        with output.open("w", encoding="utf-8") as handle:
            for row in rows:
                handle.write(json.dumps(row, ensure_ascii=False) + "\n")
        return str(output)
