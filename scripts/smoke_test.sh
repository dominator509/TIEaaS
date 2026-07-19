#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${TIE_BASE_URL:-http://127.0.0.1:8080}"

echo "==> healthz"
curl -fsS "$BASE_URL/healthz" | jq . || curl -fsS "$BASE_URL/healthz"

echo "==> readyz"
curl -fsS "$BASE_URL/readyz" | jq . || curl -fsS "$BASE_URL/readyz"

echo "==> validate code payload"
curl -fsS -X POST "$BASE_URL/v1/validate" \
  -H 'Content-Type: application/json' \
  -d '{
    "subject_type": "code",
    "subject": "fn main() { println!(\"hello tie\"); }",
    "metadata": {
      "critical": true,
      "source": "smoke-test"
    }
  }' | jq . || true
