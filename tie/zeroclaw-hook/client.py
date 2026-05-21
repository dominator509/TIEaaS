from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Optional
import json
import urllib.error
import urllib.request

from models import ValidationRequest, ValidationResponse


class TIEClientError(RuntimeError):
    pass


@dataclass(slots=True)
class TIEApiClient:
    base_url: str
    api_key: Optional[str] = None
    bearer_token: Optional[str] = None
    timeout_seconds: float = 5.0
    user_agent: str = "zeroclaw-hook/1.0"

    def _headers(self) -> Dict[str, str]:
        headers = {
            "Content-Type": "application/json",
            "Accept": "application/json",
            "User-Agent": self.user_agent,
        }
        if self.api_key:
            headers["X-API-Key"] = self.api_key
        if self.bearer_token:
            headers["Authorization"] = f"Bearer {self.bearer_token}"
        return headers

    def _request(self, method: str, path: str, payload: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        url = f"{self.base_url.rstrip('/')}{path}"
        data = None
        if payload is not None:
            data = json.dumps(payload).encode("utf-8")

        request = urllib.request.Request(
            url=url,
            method=method.upper(),
            data=data,
            headers=self._headers(),
        )

        try:
            with urllib.request.urlopen(request, timeout=self.timeout_seconds) as response:
                body = response.read().decode("utf-8")
                if not body:
                    return {}
                return json.loads(body)
        except urllib.error.HTTPError as exc:
            body = exc.read().decode("utf-8", errors="replace")
            raise TIEClientError(f"TIE HTTP {exc.code}: {body}") from exc
        except urllib.error.URLError as exc:
            raise TIEClientError(f"Unable to reach TIE service: {exc}") from exc
        except json.JSONDecodeError as exc:
            raise TIEClientError("TIE service returned invalid JSON") from exc

    def health(self) -> Dict[str, Any]:
        return self._request("GET", "/healthz")

    def readiness(self) -> Dict[str, Any]:
        return self._request("GET", "/readyz")

    def validate(self, request: ValidationRequest) -> ValidationResponse:
        payload = self._request("POST", "/v1/validate", request.to_payload())
        return ValidationResponse.from_payload(payload)
