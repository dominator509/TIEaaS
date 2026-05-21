# Kaizen Reporting

The Kaizen engine turns validation outcomes into system-improvement signals without slowing the hot request path.

## What Kaizen captures

Every validation request can emit a Kaizen event containing:

- request metadata
- artifact type
- verdict and severity
- adapter evidence summaries
- cache hit or miss information
- latency profile
- fallback or timeout markers
- downstream disposition when known

## Goals

- identify repeated failure patterns
- find false positives and false negatives
- propose rule improvements
- detect drift in verifier behavior
- prioritize registry maintenance work

## Event flow

```text
validate request -> adapter evidence -> decision -> kaizen event -> async sink -> clustering / reporting -> proposal backlog
```

## Core report types

### 1. Daily operational summary

Includes:

- total validations
- pass/warn/fail distribution
- p50/p95/p99 latency
- adapter timeout rate
- cache hit rate
- top registry namespaces involved

### 2. Failure cluster report

Includes:

- repeated rule violations by category
- most frequent blocked actions
- unresolved factual contradiction patterns
- recurring malformed inputs by client surface

### 3. Drift report

Includes:

- sudden increase in a verifier’s fail rate
- source quality deterioration for factual checks
- mismatch between advisory and fail-closed results
- regression after policy or registry updates

### 4. Proposal report

Includes:

- recommended registry additions
- candidate policy changes
- adapter threshold tuning ideas
- requests for benchmark corpus expansion

## Recommended metrics

- `tie_validation_requests_total`
- `tie_validation_latency_ms_bucket`
- `tie_validation_cache_hits_total`
- `tie_adapter_timeouts_total`
- `tie_adapter_failures_total`
- `tie_registry_reads_total`
- `tie_registry_signature_mismatch_total`
- `tie_kaizen_events_total`
- `tie_kaizen_cluster_count`

## Suggested alert thresholds

Page when:

- p95 latency exceeds 250 ms for 15 minutes
- adapter timeout rate exceeds 2% for 10 minutes
- readiness failures exceed 3 consecutive checks

Non-paging alert when:

- cache hit rate drops below 40% for one hour
- warning rate doubles from the 7-day baseline
- signature mismatch count is non-zero

## Governance loop

1. Review daily operational summary.
2. Triage the highest-value clusters.
3. Decide whether the fix belongs in policy, registry, or adapter logic.
4. Add benchmark cases before changing production rules.
5. Roll out policy changes behind advisory or canary modes first.
6. Compare pre-change and post-change Kaizen metrics.

## Commercial guidance

- Never auto-apply Kaizen proposals directly to production policy.
- Require human approval for rule changes and registry edits.
- Keep proposal IDs, approver identity, and rollout timestamps in the audit trail.
- Export reports to the same analytics surface used by platform operations.
