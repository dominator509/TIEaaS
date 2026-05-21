# ZeroClaw Integration Guide

This guide explains how to place TIE in front of ZeroClaw output release so LLM responses are validated before they are shown, executed, or persisted.

## Files

The hook package is in `zeroclaw-hook/`:

- `tie_validator.py` — primary interception logic
- `client.py` — standard-library HTTP client for TIE
- `memory_store.py` — local SQLite memory for verdict history
- `models.py` — shared dataclasses and payload models

## Deployment pattern

```text
ZeroClaw generation -> TIE hook -> /v1/validate -> verdict -> allow/block/quarantine -> memory persistence
```

## Basic configuration

```bash
export TIE_BASE_URL="http://localhost:8080"
export TIE_API_KEY="replace-me"
export TIE_POLICY_MODE="critical-fail-closed"
export TIE_MEMORY_DB="./zeroclaw_validation_reports.db"
export TIE_FAIL_ON_CLIENT_ERROR="false"
```

## Example wrapper

```python
from tie_validator import TieValidator, wrap_generation

validator = TieValidator.from_env()

def generate(prompt: str) -> str:
    return f"Model output for: {prompt}"

safe_generate = wrap_generation(generate, validator)

result = safe_generate("Summarize the deployment runbook")
print(result)
```

## Recommended release policies

### Advisory

Use for early rollout, eval environments, or trust-calibration phases.

- TIE always returns a report.
- ZeroClaw releases the model output even when warnings exist.
- Validation reports are written to memory for later Kaizen analysis.

### Critical-fail-closed

Recommended default.

- Critical policy violations block release.
- Low-severity issues are logged and surfaced as warnings.
- TIE client errors can be configured fail-open during non-production rollout.

### Full-fail-closed

Use for regulated or action-heavy environments.

- Any blocking verifier failure prevents output release.
- TIE client errors should be fail-closed.
- Release path should present a user-safe fallback message.

## Memory storage

The SQLite memory store is designed to retain:

- request ID
- artifact type
- verdict and severity
- evidence summary
- policy mode
- timestamp
- client error state if applicable

### Suggested retention policy

- Hot operational history: 7 to 30 days
- Kaizen analysis snapshots: 90 days or more
- Export long-term reports to object storage or the central observability stack

## Fallback behavior

ZeroClaw must handle two classes of failure separately:

1. TIE returns a real verdict.
2. TIE cannot be reached or returns an upstream error.

Recommended policy:

- Non-production: fail-open on client errors, but log and persist all incidents.
- Production with low risk: fail-open only for read-only factual workflows.
- Production with high risk: fail-closed for any execution, side effects, or regulated content.

## Integration checklist

- Inject request metadata such as conversation ID, user ID, model name, and prompt hash.
- Pass any supporting citations or action context in the artifact payload.
- Store TIE request IDs with ZeroClaw traces.
- Surface TIE warnings in internal tooling even when output is allowed.
- Route blocked outputs to a reviewer queue when human override is allowed.
