from __future__ import annotations

from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from typing import Any, Dict, List, Literal, Optional
import json
import uuid

ValidationSubjectType = Literal["code", "fact", "action"]
ValidationVerdict = Literal["pass", "warn", "fail", "inconclusive"]
EnforcementMode = Literal["advisory", "critical_fail_closed", "full_fail_closed"]


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


@dataclass(slots=True)
class Evidence:
    kind: str
    value: str
    uri: Optional[str] = None
    metadata: Dict[str, Any] = field(default_factory=dict)


@dataclass(slots=True)
class ValidationRequest:
    subject_type: ValidationSubjectType
    subject: str
    policy_profile: Optional[str] = None
    enforcement_mode: Optional[str] = None
    critical: bool = False
    request_id: str = field(default_factory=lambda: str(uuid.uuid4()))
    metadata: Dict[str, Any] = field(default_factory=dict)
    evidence: List[Evidence] = field(default_factory=list)

    def to_payload(self) -> Dict[str, Any]:
        return {
            "request_id": self.request_id,
            "subject_type": self.subject_type,
            "subject": self.subject,
            "policy_profile": self.policy_profile,
            "enforcement_mode": self.enforcement_mode,
            "critical": self.critical,
            "metadata": self.metadata,
            "evidence": [asdict(item) for item in self.evidence],
        }


@dataclass(slots=True)
class ValidationResponse:
    request_id: str
    verdict: ValidationVerdict
    score: Optional[float] = None
    reasons: List[str] = field(default_factory=list)
    warnings: List[str] = field(default_factory=list)
    validator_results: List[Dict[str, Any]] = field(default_factory=list)
    signed_verdict_token: Optional[str] = None
    raw: Dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_payload(cls, payload: Dict[str, Any]) -> "ValidationResponse":
        return cls(
            request_id=str(payload.get("request_id", "")),
            verdict=payload.get("verdict", "inconclusive"),
            score=payload.get("score"),
            reasons=list(payload.get("reasons", [])),
            warnings=list(payload.get("warnings", [])),
            validator_results=list(payload.get("validator_results", [])),
            signed_verdict_token=payload.get("signed_verdict_token"),
            raw=payload,
        )


@dataclass(slots=True)
class ValidationReport:
    report_id: str = field(default_factory=lambda: str(uuid.uuid4()))
    created_at: str = field(default_factory=utc_now_iso)
    request: ValidationRequest = field(default_factory=lambda: ValidationRequest(subject_type="fact", subject=""))
    response: Optional[ValidationResponse] = None
    provider: Optional[str] = None
    model: Optional[str] = None
    conversation_id: Optional[str] = None
    agent_id: Optional[str] = None
    user_id: Optional[str] = None
    blocked: bool = False
    block_reason: Optional[str] = None
    released_output: Optional[str] = None

    def to_record(self) -> Dict[str, Any]:
        return {
            "report_id": self.report_id,
            "created_at": self.created_at,
            "provider": self.provider,
            "model": self.model,
            "conversation_id": self.conversation_id,
            "agent_id": self.agent_id,
            "user_id": self.user_id,
            "blocked": self.blocked,
            "block_reason": self.block_reason,
            "released_output": self.released_output,
            "request_id": self.request.request_id,
            "subject_type": self.request.subject_type,
            "request_payload": json.dumps(self.request.to_payload(), ensure_ascii=False),
            "response_payload": json.dumps(self.response.raw if self.response else {}, ensure_ascii=False),
            "verdict": self.response.verdict if self.response else None,
            "score": self.response.score if self.response else None,
        }
