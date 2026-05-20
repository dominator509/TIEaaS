#!/usr/bin/env bash
set -euo pipefail

cargo run -- registry create \
  --namespace facts \
  --kind citation_policy \
  --key default \
  --value '{"require_citations":true}' \
  --provenance '{"seed":"scripts/seed_registry.sh"}' \
  --tags seed,default
