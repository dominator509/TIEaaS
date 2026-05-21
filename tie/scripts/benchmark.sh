#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${TIE_BASE_URL:-http://127.0.0.1:8080}"
REQUESTS="${REQUESTS:-50}"

for i in $(seq 1 "$REQUESTS"); do
  curl -s -o /dev/null -X POST "$BASE_URL/v1/validate" \
    -H 'Content-Type: application/json' \
    -d '{
      "kind": "fact",
      "subject": {"claim": "Rust provides memory safety without a garbage collector."},
      "evidence": [{"type": "citation", "uri": "https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html"}],
      "context": {"critical": false, "source": "benchmark"}
    }'
done

echo "Sent $REQUESTS requests to $BASE_URL"
