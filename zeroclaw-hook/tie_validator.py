from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, Literal, Optional
import hashlib
import os

from client import TIEApiClient, TIEClientError
from memory_store import SQLiteMemoryStore
from models import Evidence, ValidationReport, ValidationRequest, ValidationResponse

SubjectType = Literal["code", "fact", "action"]
EnforcementMode = Literal["advisory", "critical_fail_closed", "full_fail_closed"]


class TIEValidationBlockedError(RuntimeError):
    pass


@dataclass(slots=True)
class InterceptDecision:
    allowed: bool
    blocked: bool
    verdict: str
    request_id: str
    reasons: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)
    signed_verdict_token: Optional[str] = None
    released_output: Optional[str] = None


@dataclass(slots=True)
class TIEValidationHook:
    client: TIEApiClient
    memory_store: SQLiteMemoryStore
    default_subject_type: SubjectType = "fact"
    default_enforcement_mode: EnforcementMode = "critical_fail_closed"
    policy_profile: Optional[str] = None
    fail_open_on_client_error: bool = False

    @classmethod
    def from_env(cls) -> "TIEValidationHook":
        client = TIEApiClient(
            base_url=os.environ.get("TIE_BASE_URL", "http://localhost:8080"),
            api_key=os.environ.get("TIE_API_KEY"),
            bearer_token=os.environ.get("TIE_BEARER_TOKEN"),
            timeout_seconds=float(os.environ.get("TIE_TIMEOUT_SECONDS", "5.0")),
        )
        store = SQLiteMemoryStore(
            database_path=os.environ.get(
                "TIE_REPORT_DB_PATH",
                "./zeroclaw-hook/tie_validation_reports.db",
            )
        )
        return cls(
            client=client,
            memory_store=store,
            default_subject_type=os.environ.get("TIE_SUBJECT_TYPE", "fact"),
            default_enforcement_mode=os.environ.get(
                "TIE_ENFORCEMENT_MODE",
                "critical_fail_closed",
            ),
            policy_profile=os.environ.get("TIE_POLICY_PROFILE"),
            fail_open_on_client_error=os.environ.get("TIE_FAIL_OPEN_ON_CLIENT_ERROR", "false").lower() == "true",
        )

    def intercept_output(
        self,
        output_text: str,
        *,
        subject_type: Optional[SubjectType] = None,
        enforcement_mode: Optional[EnforcementMode] = None,
        critical: bool = False,
        provider: Optional[str] = None,
        model: Optional[str] = None,
        conversation_id: Optional[str] = None,
        agent_id: Optional[str] = None,
        user_id: Optional[str] = None,
        metadata: Optional[Dict[str, Any]] = None,
        extra_evidence: Optional[list[Evidence]] = None,
    ) -> InterceptDecision:
        metadata = dict(metadata or {})
        metadata.setdefault("output_sha256", hashlib.sha256(output_text.encode("utf-8")).hexdigest())
        metadata.setdefault("output_length", len(output_text))
        metadata.setdefault("integration", "zeroclaw-hook")

        request = ValidationRequest(
            subject_type=subject_type or self.default_subject_type,
            subject=output_text,
            policy_profile=self.policy_profile,
            enforcement_mode=enforcement_mode or self.default_enforcement_mode,
            critical=critical,
            metadata=metadata,
            evidence=list(extra_evidence or []),
        )

        report = ValidationReport(
            request=request,
            provider=provider,
            model=model,
            conversation_id=conversation_id,
            agent_id=agent_id,
            user_id=user_id,
        )

        try:
            response = self.client.validate(request)
            report.response = response
            allowed = self._is_allowed(response, request.enforcement_mode or self.default_enforcement_mode, critical)
            report.blocked = not allowed
            report.block_reason = None if allowed else self._build_block_reason(response)
            report.released_output = output_text if allowed else None
            self.memory_store.save_report(report)
        except TIEClientError as exc:
            if not self.fail_open_on_client_error:
                report.blocked = True
                report.block_reason = f"TIE validation unavailable: {exc}"
                self.memory_store.save_report(report)
                raise TIEValidationBlockedError(report.block_reason) from exc

            report.blocked = False
            report.block_reason = None
            report.released_output = output_text
            self.memory_store.save_report(report)
            return InterceptDecision(
                allowed=True,
                blocked=False,
                verdict="inconclusive",
                request_id=request.request_id,
                warnings=[f"Validation service unavailable: {exc}"],
                released_output=output_text,
            )

        if report.blocked:
            raise TIEValidationBlockedError(report.block_reason or "Output blocked by TIE")

        return InterceptDecision(
            allowed=True,
            blocked=False,
            verdict=response.verdict,
            request_id=response.request_id,
            reasons=response.reasons,
            warnings=response.warnings,
            signed_verdict_token=response.signed_verdict_token,
            released_output=output_text,
        )

    def wrap_generation(self, generation_fn, **fixed_context: Any):
        def wrapped(*args: Any, **kwargs: Any) -> Any:
            output = generation_fn(*args, **kwargs)
            if not isinstance(output, str):
                raise TypeError("ZeroClaw wrapped generation function must return a string")
            self.intercept_output(output, **fixed_context)
            return output

        return wrapped

    def _is_allowed(
        self,
        response: ValidationResponse,
        enforcement_mode: EnforcementMode,
        critical: bool,
    ) -> bool:
        verdict = response.verdict
        if enforcement_mode == "advisory":
            return True
        if verdict == "pass":
            return True
        if verdict == "warn":
            return enforcement_mode != "full_fail_closed"
        if enforcement_mode == "critical_fail_closed":
            return not critical
        return False

    @staticmethod
    def _build_block_reason(response: ValidationResponse) -> str:
        parts = []
        if response.reasons:
            parts.extend(response.reasons)
        if response.warnings and not parts:
            parts.extend(response.warnings)
        if not parts:
            parts.append(f"Validation verdict was {response.verdict}")
        return "; ".join(parts)


if __name__ == "__main__":
    hook = TIEValidationHook.from_env()
    sample = "The moon is made of cheese."
    try:
        decision = hook.intercept_output(
            sample,
            subject_type="fact",
            provider="demo",
            model="demo-model",
            conversation_id="local-demo",
            metadata={"channel": "manual-test"},
        )
        print({
            "allowed": decision.allowed,
            "verdict": decision.verdict,
            "request_id": decision.request_id,
            "warnings": decision.warnings,
        })
    except TIEValidationBlockedError as exc:
        print({"allowed": False, "reason": str(exc)})
