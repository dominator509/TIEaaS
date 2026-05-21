# TIE API Usage

This guide shows how to call the TIE HTTP API, interpret responses, and integrate validation into application flows.

## Base URL

```bash
export TIE_BASE_URL="http://localhost:8080"
export TIE_API_KEY="replace-me"
```

The current implementation exposes HTTP endpoints from the single-binary service in `src/main.rs`.

## Health and readiness

Health checks should be used by load balancers and orchestration platforms.

```bash
curl -s "$TIE_BASE_URL/healthz"
curl -s "$TIE_BASE_URL/readyz"
```

## Validate a code artifact

```bash
curl -sS -X POST "$TIE_BASE_URL/v1/validate" \
  -H "Content-Type: application/json" \
  -H "X-API-Key: $TIE_API_KEY" \
  -d '{
    "artifact_type": "code",
    "artifact": {
      "language": "rust",
      "content": "fn add(a:i32,b:i32)->i32{a+b}"
    },
    "registry_refs": ["policy/code/default"],
    "metadata": {
      "source": "cli",
      "requestor": "developer@example.com"
    }
  }'
```

Typical response shape:

```json
{
  "request_id": "0196c0f2-2e6f-7bf1-8b4e-0242ac120002",
  "verdict": "pass",
  "severity": "info",
  "policy_mode": "critical-fail-closed",
  "evidence": [
    {
      "adapter": "code_verifier",
      "status": "pass",
      "message": "Artifact passed baseline structural checks"
    }
  ],
  "timings_ms": {
    "total": 22,
    "cache_lookup": 1,
    "verification": 18,
    "decision": 3
  }
}
```

## Validate a factual response

```bash
curl -sS -X POST "$TIE_BASE_URL/v1/validate" \
  -H "Content-Type: application/json" \
  -H "X-API-Key: $TIE_API_KEY" \
  -d '{
    "artifact_type": "fact",
    "artifact": {
      "claim": "The Eiffel Tower is in Paris.",
      "citations": [
        {"source": "registry", "ref": "facts/landmarks/eiffel_tower"}
      ]
    },
    "metadata": {
      "source": "zeroclaw",
      "conversation_id": "conv_123"
    }
  }'
```

## Validate an action proposal

```bash
curl -sS -X POST "$TIE_BASE_URL/v1/validate" \
  -H "Content-Type: application/json" \
  -H "X-API-Key: $TIE_API_KEY" \
  -d '{
    "artifact_type": "action",
    "artifact": {
      "action": "deploy_service",
      "target": "payments-api",
      "environment": "production",
      "requested_by": "workflow-bot"
    },
    "registry_refs": ["runbooks/deploy/prod"],
    "metadata": {
      "source": "n8n"
    }
  }'
```

## Error handling

TIE responds with a canonical error envelope:

```json
{
  "error": {
    "code": "invalid_input",
    "message": "invalid input: key must not be empty",
    "retryable": false,
    "request_id": "0196c0f2-31be-7654-a839-0242ac120002"
  }
}
```

### Recommended client behavior

- Retry on `timeout`, `database_error`, and `internal_error` only when `retryable=true`.
- Do not retry `invalid_input` or `not_found` without changing the request.
- Log `request_id` in every downstream system.
- When the deployment is `critical-fail-closed` or `full-fail-closed`, treat non-pass verdicts as execution blockers.

## Ground Truth Registry CRUD

### Create a record

```bash
curl -sS -X POST "$TIE_BASE_URL/v1/registry/records" \
  -H "Content-Type: application/json" \
  -H "X-API-Key: $TIE_API_KEY" \
  -d '{
    "namespace": "facts",
    "kind": "canonical_fact",
    "key": "landmarks/eiffel_tower",
    "value": {"city": "Paris", "country": "France"},
    "provenance": {"source": "editorial", "owner": "trust-team"},
    "tags": ["landmark", "france"]
  }'
```

### List records

```bash
curl -sS "$TIE_BASE_URL/v1/registry/records?include_retired=false" \
  -H "X-API-Key: $TIE_API_KEY"
```

### Get by ID

```bash
curl -sS "$TIE_BASE_URL/v1/registry/records/<record-id>" \
  -H "X-API-Key: $TIE_API_KEY"
```

### Get latest by namespace/kind/key

```bash
curl -sS "$TIE_BASE_URL/v1/registry/lookup/facts/canonical_fact/landmarks%2Feiffel_tower" \
  -H "X-API-Key: $TIE_API_KEY"
```

### Update

```bash
curl -sS -X PUT "$TIE_BASE_URL/v1/registry/records/<record-id>" \
  -H "Content-Type: application/json" \
  -H "X-API-Key: $TIE_API_KEY" \
  -d '{
    "value": {"city": "Paris", "country": "France", "verified": true},
    "provenance": {"source": "editorial", "updated_by": "trust-team"},
    "tags": ["landmark", "france", "verified"]
  }'
```

### Soft delete

```bash
curl -sS -X DELETE "$TIE_BASE_URL/v1/registry/records/<record-id>" \
  -H "X-API-Key: $TIE_API_KEY"
```

## Operational recommendations

- Put TIE behind an API gateway or service mesh that enforces API key or JWT authentication.
- Cache identical validation calls at the client only when requests are immutable and idempotent.
- For long-running proof workflows, use webhooks or async orchestration rather than blocking user-facing request paths.
- Mirror the OpenAPI schema in client SDK generation to reduce drift.
