# TIE Service Architecture

## Purpose

TIE (Trust, Inspection, and Enforcement) is a validation and gating service designed to inspect code, facts, and actions before downstream execution. The v1 target is **single-tenant deployment** with **multi-tenant-ready seams** so the system can scale to enterprise or SaaS models later without major architectural change.

The architecture is optimized for:

- **Low-latency validation** on the hot path
- **Configurable enforcement modes** at deployment time
- **Pluggable verifier adapters** for code, fact, and action validation
- **Ground-truth-backed decisions** with provenance and signature support
- **Kaizen feedback loops** that continuously improve rules, policies, and benchmark coverage

## Deployment Profile Assumptions

### v1 default
- Single-tenant
- HTTP API first
- SQLite-backed registry for local simplicity
- Rust monorepo with clear module boundaries
- n8n node, ZeroClaw hook, CLI, and webhooks as first-class integrations

### Future-ready seams
- Optional tenant namespace on all registry and audit entities
- API gateway boundary that can front multiple service instances later
- Adapter execution model that can be moved from in-process to isolated workers
- Registry abstraction that can later swap from SQLite to Postgres or distributed storage

---

## System Diagram

```text
                                  ┌──────────────────────────┐
                                  │       External Users     │
                                  │  Apps / Agents / Teams   │
                                  └────────────┬─────────────┘
                                               │
                           validate request / webhook / cli / hook call
                                               │
     ┌──────────────────────┬──────────────────┼──────────────────┬──────────────────────┐
     │                      │                  │                  │                      │
     ▼                      ▼                  ▼                  ▼                      ▼
┌──────────────┐      ┌──────────────┐   ┌───────────┐    ┌──────────────┐       ┌──────────────┐
│  n8n Node    │      │ ZeroClaw Hook│   │    CLI    │    │   Webhooks   │       │  OpenClaw    │
│  (workflow)  │      │ (LLM output  │   │ tie-cli   │    │ inbound/out  │       │ integration  │
│              │      │ interceptor) │   │           │    │              │       │ seam         │
└──────┬───────┘      └──────┬───────┘   └─────┬─────┘    └──────┬───────┘       └──────┬───────┘
       │                     │                 │                  │                      │
       └─────────────────────┴─────────────────┴──────────────────┴──────────────────────┘
                                               │
                                               ▼
                                  ┌──────────────────────────┐
                                  │  API Gateway             │
                                  │  HTTP now, gRPC-ready    │
                                  │  Auth / Rate Limit / IDs │
                                  └────────────┬─────────────┘
                                               │ normalized validation request
                                               ▼
                                  ┌──────────────────────────┐
                                  │      TIE Service Core    │
                                  │ Request Router           │
                                  │ Policy Engine            │
                                  │ Validation Orchestrator  │
                                  │ Result Composer          │
                                  └───────┬────────┬─────────┘
                                          │        │
                         ground-truth read│        │verification dispatch
                                          │        │
                                          ▼        ▼
                           ┌──────────────────┐   ┌──────────────────────────────┐
                           │ Ground Truth     │   │     Verifier Adapters        │
                           │ Registry         │   │                              │
                           │ specs / code /   │   │  ┌────────────────────────┐  │
                           │ facts / actions  │   │  │ Code Verifier Adapter   │  │
                           │ signed records   │   │  │ lint / static / SMT     │  │
                           └────────┬─────────┘   │  └────────────────────────┘  │
                                    │             │  ┌────────────────────────┐  │
                                    │ registry    │  │ Fact Verifier Adapter   │  │
                                    │ response    │  │ provenance / conflict   │  │
                                    │             │  └────────────────────────┘  │
                                    │             │  ┌────────────────────────┐  │
                                    │             │  │ Action Verifier Adapter │  │
                                    │             │  │ policy / preconditions  │  │
                                    │             │  └────────────────────────┘  │
                                    │             └──────────────┬───────────────┘
                                    │                            │ adapter verdicts
                                    └──────────────┬─────────────┘
                                                   ▼
                                  ┌──────────────────────────┐
                                  │      Decision Layer      │
                                  │ pass / warn / fail /     │
                                  │ andon / retry / quarantine│
                                  └────────────┬─────────────┘
                                               │ validation response
                                               ▼
     ┌──────────────────────┬──────────────────┼──────────────────┬──────────────────────┐
     │                      │                  │                  │                      │
     ▼                      ▼                  ▼                  ▼                      ▼
┌──────────────┐      ┌──────────────┐   ┌───────────┐    ┌──────────────┐       ┌──────────────┐
│  n8n Node    │      │ ZeroClaw Hook│   │    CLI    │    │   Webhooks   │       │  OpenClaw    │
│ report + next│      │ allow/block  │   │ terminal  │    │ callback     │       │ result       │
│ workflow step│      │ + memory save│   │ report    │    │ delivery     │       │              │
└──────────────┘      └──────────────┘   └───────────┘    └──────────────┘       └──────────────┘

                                                   │
                                                   │ kaizen event / failures / drift signals
                                                   ▼
                                  ┌──────────────────────────┐
                                  │       Kaizen Engine      │
                                  │ Pattern mining           │
                                  │ Error clustering         │
                                  │ Rule proposal generation │
                                  │ Benchmark feedback       │
                                  └────────────┬─────────────┘
                                               │ signed proposal / policy update candidate
                                               ▼
                                  ┌──────────────────────────┐
                                  │ Ground Truth Registry    │
                                  │ + Policy Config Store    │
                                  └──────────────────────────┘
```

---

## Core Components

### 1. API Gateway (HTTP now, gRPC-ready)

The gateway is the stable ingress boundary for all external clients and adapters.

**Responsibilities**
- Accept validation requests over HTTP
- Preserve a clean internal boundary that can expose gRPC later
- Perform authentication, request normalization, and rate limiting
- Assign correlation IDs, trace IDs, and verdict IDs
- Enforce payload size and adapter budget policies

**Design note**
The initial implementation can be embedded in the same Rust process as the service core, but the interface should be expressed as a boundary so it can later become a standalone gateway.

### 2. TIE Service Core

The service core is the orchestrator and policy-enforcement heart of TIE.

**Responsibilities**
- Parse validation requests
- Classify request type: code, fact, action, or mixed
- Load applicable policies and registry context
- Dispatch to one or more verifier adapters
- Aggregate evidence and normalize verdicts
- Produce the final enforcement decision

**Internal submodules**
- Request router
- Validation orchestrator
- Policy engine
- Decision engine
- Result composer
- Audit/event emitter

### 3. Ground Truth Registry

The registry is the authoritative store for validation context.

**Supported day-one truth classes**
- Specifications
- Architectural decision records
- Verified code fragments or signatures
- Known fact packs and citation bundles
- Allowed actions, runbooks, and execution policies
- Benchmark cases and adjudicated outcomes

**Responsibilities**
- Store versioned truth records
- Preserve provenance metadata
- Support Ed25519 signatures for tamper-evident records
- Return scoped truth sets by domain, profile, and policy
- Separate hot-path indexes from archival evidence blobs

### 4. Verifier Adapters

Adapters are pluggable validation engines behind a common contract.

#### Code Verifier Adapter
Validates source code, generated code, or code-related claims.

**Examples**
- Rust lint or static analysis
- Formal or semi-formal verification hooks
- Z3-backed property checks
- Contract/schema conformance

#### Fact Verifier Adapter
Validates claims against truth packs or trusted sources.

**Examples**
- Citation presence and provenance checks
- Source ranking and confidence scoring
- Contradiction detection against registry facts

#### Action Verifier Adapter
Validates whether an action is allowed, safe, and policy-conformant.

**Examples**
- Runbook matching
- Required approvals present
- Idempotency and replay protection checks
- Execution precondition validation

### 5. Decision Layer

The decision layer translates adapter results into deployment-enforced outcomes.

**Supported outcomes**
- `pass`
- `warn`
- `fail`
- `retry`
- `quarantine`
- `andon`

**Deployment-configurable enforcement modes**
- `advisory`: never block, always return report
- `critical_fail_closed`: block on failed critical checks only
- `full_fail_closed`: block on any failed mandatory check

### 6. Kaizen Engine

The Kaizen Engine is the continuous improvement subsystem.

**Responsibilities**
- Capture failed validations and near-miss patterns
- Detect drift in verifier quality or registry coverage
- Cluster similar errors for rule proposals
- Generate benchmark candidates and policy recommendations
- Feed approved improvements back into the registry

---

## Ground Truth Registry Strategy

The Ground Truth Registry is the most important trust-bearing subsystem in TIE. It must be treated as a governed product, not just a database.

### Registry record model

Each truth record should contain:
- `record_id`
- `namespace`
- `record_type` such as `spec`, `fact_pack`, `runbook`, `policy`, `benchmark_case`
- `subject_ref`
- `version`
- `status` such as `draft`, `active`, `deprecated`, `revoked`
- `provenance`
- `signature`
- `effective_from` and optional `effective_to`
- `confidence_class`
- `conflict_set_id` when mutually exclusive records exist
- `content_pointer` to inline content or blob storage

### Bootstrap strategy

Phase 0 must define how trustworthy data enters the system for the first time.

**Bootstrap sources**
- Approved specifications and design docs
- Existing runbooks and SOPs
- Curated benchmark and regression cases
- Known-safe code samples and policy exemplars
- Approved external fact packs with provenance snapshots

**Bootstrap workflow**
1. Import candidate artifacts into a staging area.
2. Normalize them into registry record types.
3. Require human approval for first activation.
4. Sign the activated version.
5. Publish a registry snapshot manifest.

**Commercial rule**
No artifact becomes active truth merely because it was uploaded. It must cross a governance boundary.

### Contradiction resolution

Contradictions are inevitable. TIE must resolve them deterministically.

**Resolution order**
1. Explicitly scoped, environment-matching policy beats generic policy.
2. Newer active version beats older active version when same issuer and same subject.
3. Higher-trust issuer beats lower-trust issuer.
4. Human-approved override beats auto-generated proposal.
5. Unresolved conflicts downgrade verdict confidence and can force `warn` or `quarantine` depending on policy.

**Conflict handling modes**
- `strict_conflict_fail`: any unresolved contradiction is a failure on critical subjects
- `strict_conflict_warn`: contradiction returns a warning and confidence downgrade
- `prefer_latest_signed`: choose latest signed active record automatically

### Long-term maintenance model

The registry needs lifecycle governance.

**Maintenance controls**
- Versioning on every mutation
- Soft deprecation before revocation where possible
- Snapshot export for rollback
- Tombstones for removed records
- Drift reports from Kaizen when truth coverage is weak or stale
- Review cadences by record type, for example runbooks every 90 days and fact packs every 30 days

### Scaling path

**v1**
- SQLite for single-tenant simplicity
- WAL mode enabled
- hot-path tables indexed for namespace, record_type, subject_ref, status, version

**v2**
- Postgres for concurrent writers and operational scale
- object/blob storage for evidence bundles and large fact packs
- read replicas for heavy query load

**v3**
- optional distributed registry API
- shard by tenant or domain namespace
- offline snapshot signing and replication

### Registry read policy for hot path

The hot path should never scan the full registry. It should read only:
- active records
- matching namespaces
- permitted record types
- most recent effective versions
- pre-built index bundles for common validation classes

---

## Verifier Adapter Contract and Execution Model

Adapters must behave like bounded, deterministic services, even when implemented in-process.

### Standard adapter contract

Each adapter should return:
- `adapter_name`
- `adapter_version`
- `status`: `pass`, `warn`, `fail`, `inconclusive`, `timeout`, `error`
- `severity`
- `confidence`
- `evidence_refs`
- `findings`
- `timing_ms`
- `cache_hit`
- `budget_used`

### Execution guarantees

Every adapter must declare:
- maximum execution budget
- concurrency profile
- cache key strategy
- deterministic input normalization rules
- fallback behavior

### Fallback behavior

**Required fallback cases**
- `timeout`: adapter exceeded budget
- `dependency_unavailable`: external prover or source unavailable
- `registry_conflict`: ground truth was ambiguous
- `unsupported_input`: subject class not supported

**Policy behavior**
- In `advisory`, timeouts degrade to warnings unless adapter is explicitly critical.
- In `critical_fail_closed`, critical adapter timeout blocks execution.
- In `full_fail_closed`, any mandatory adapter timeout blocks execution.

### Cache strategy

The adapter layer should support three caches:
- **Request normalization cache** keyed by normalized subject hash
- **Verdict cache** keyed by subject hash + policy profile + truth snapshot ID
- **Evidence cache** keyed by external artifact hash or source manifest

A cache entry must be invalidated when:
- policy profile changes
- truth snapshot changes
- adapter version changes
- input normalization rules change

### Progressive verification tiers

TIE should not send every request directly to the most expensive verifier.

**Tier 0: normalization and cheap guards**
- schema validation
- policy lookup
- hash computation
- replay detection
- known-blocklist / known-allowlist lookup

**Tier 1: fast heuristics**
- lint-like checks
- citation presence checks
- runbook precondition matching
- regex and structural validation

**Tier 2: deep deterministic checks**
- cross-record contradiction checks
- benchmark lookups
- semantic consistency rules
- lightweight symbolic checks

**Tier 3: expensive formal checks**
- SMT solving
- proof adapters
- external source refresh and adjudication
- long-running action safety proofs

The orchestrator should stop early when policy permits and sufficient confidence is achieved.

---

## Performance and Latency Strategy

TIE must be designed around validation speed, not just correctness.

### Latency budgets

Suggested v1 service-level targets:
- **p50** hot-path validation under 40 ms for cached/simple requests
- **p95** under 150 ms for common policy-backed checks
- **p99** under 500 ms for mixed validations without formal solver escalation
- **async/offloaded** path for long-running proofs beyond 500 ms

### Budget allocation model

Example per-request budget:
- gateway/auth/normalization: 5–10 ms
- registry read: 5–20 ms
- fast adapter pass: 20–60 ms
- result assembly and audit emit: 5–10 ms

### Throughput strategy

- Normalize once, fan out many
- Parallelize independent adapters
- Keep Kaizen fully off the synchronous hot path
- Reuse registry snapshots for batches
- Cache identical requests aggressively with truth snapshot awareness
- Use bounded worker pools for expensive verifier classes

### Async escalation model

When a request exceeds synchronous budget:
1. return fast verdict if policy allows provisional result
2. enqueue deep verification job
3. issue webhook or pollable result handle
4. persist final adjudication and notify caller

### Performance safeguards

- hard timeouts per adapter
- queue depth limits
- circuit breaker around unstable external dependencies
- backpressure when solver pool is saturated
- request body size caps
- input deduplication for repeated validations in burst windows

---

## Security and Authentication Model

TIE is a trust boundary and must be treated like a control-plane service.

### Authentication

Support these auth modes from the start:
- API keys for service-to-service integrations
- JWT bearer validation for platform environments
- local dev tokens for CLI and single-node development
- optional mTLS for internal deployments

### Authorization

Even in single-tenant mode, authorization should be scoped by role:
- `validator.submit`
- `registry.read`
- `registry.write`
- `policy.admin`
- `kaizen.review`
- `audit.read`

### Secrets management

- no secrets in source control
- environment or external secret store only
- signer key isolation for registry signing
- webhook secret rotation
- key IDs on every signed artifact and verdict

### Signed verdicts

TIE should produce signed verdict tokens for downstream enforcement systems.

**Signed token should include**
- verdict ID
- subject hash
- policy profile
- truth snapshot ID
- expiry time
- signature key ID

Downstream executors should be able to reject actions lacking a valid verdict token.

### Audit logging and retention

Audit records must be append-only and tamper-evident.

**Minimum audit fields**
- who submitted
- what was validated
- which policies applied
- which truth snapshot was used
- which adapters ran
- final verdict
- timings
- override events

**Retention policy**
- configurable by deployment
- default 90-day hot retention
- long-term export to cold storage for regulated environments

---

## Production Operations

Commercial readiness requires explicit operational design, not just code boundaries.

### Observability

TIE should emit:
- structured logs
- request traces with correlation IDs
- per-adapter timings
- registry query metrics
- cache hit ratios
- queue depth and worker saturation metrics
- verdict distribution and false-positive/false-negative tracking

### Core SLOs

- validation availability
- latency by subject type
- adapter timeout rate
- registry read error rate
- webhook delivery success rate
- signed verdict issuance success rate

### Resilience model

- health endpoints for liveness and readiness
- startup config linting
- circuit breakers around unstable dependencies
- retry policy only for idempotent internal calls
- dead-letter queues for async validations and webhook failures
- graceful degradation to advisory mode only when explicitly permitted by policy

### Backup and restore

Registry durability plan must exist before GA.

**Required capabilities**
- scheduled SQLite snapshot or logical export
- signed snapshot manifests
- restore drill documentation
- backup verification jobs
- point-in-time migration path when moving to Postgres

### Runbooks

Minimum runbooks required before launch:
- registry corruption recovery
- signer key rotation
- solver pool saturation
- webhook replay attack response
- cache invalidation after policy change
- emergency fail-open versus fail-closed decision procedure

### Deployment topology

**v1 suggested topology**
- one TIE service instance
- one local SQLite registry volume
- optional sidecar or background worker for async proofs and webhook delivery
- reverse proxy or ingress in front for TLS and rate limiting

**Scale-up topology**
- stateless TIE API instances
- dedicated verifier worker pool
- shared Postgres registry
- message queue for async deep verification
- object store for evidence bundles

---


## Observability Stack and Alerting Blueprint

Commercial operation needs not only telemetry primitives, but an opinionated stack and a default dashboard pack.

### Recommended stack

- **OpenTelemetry** for traces, metrics, and log correlation
- **Prometheus** for metrics scrape and alert rule evaluation
- **Grafana** for dashboards and SLO visualization
- **Loki** or equivalent for structured log aggregation
- **Tempo** or Jaeger for distributed trace storage
- **Alertmanager** for paging, routing, and suppression policy

### Required service dashboards

**1. Executive service health dashboard**
- request rate by integration surface
- success/warn/fail/inconclusive rate
- p50, p95, p99 latency
- uptime and error budget burn

**2. Hot-path performance dashboard**
- gateway auth latency
- registry read latency
- adapter execution latency by class
- cache hit/miss ratio
- synchronous vs async escalation rate

**3. Verifier quality dashboard**
- false-positive trend
- false-negative trend
- timeout rate by adapter
- inconclusive rate by adapter
- benchmark regression pass rate

**4. Registry health dashboard**
- active record count by type
- conflict-set count
- snapshot publish time
- signature verification failures
- stale record review backlog

**5. Async operations dashboard**
- queue depth
- job age percentiles
- dead-letter rate
- webhook success/failure rate
- retry volume

### Minimum alert thresholds

**Page immediately**
- p95 synchronous validation latency > 250 ms for 10 minutes
- p99 synchronous validation latency > 750 ms for 5 minutes
- registry read error rate > 1% for 5 minutes
- signed verdict issuance failure > 0.5% for 5 minutes
- critical adapter timeout rate > 2% for 10 minutes
- dead-letter queue growth sustained for 15 minutes
- audit log write failure on any production node

**Ticket / non-paging**
- cache hit rate drops below 70% for a major request class over 1 hour
- conflict-set count grows more than 20% week over week
- stale truth review backlog exceeds policy threshold
- benchmark false-negative rate drifts above target
- webhook delivery retry rate exceeds 5% over 30 minutes

### Metric naming guidance

Use stable names from day one so dashboards and alerts survive refactors:
- `tie_request_total`
- `tie_request_latency_ms`
- `tie_registry_read_latency_ms`
- `tie_adapter_latency_ms`
- `tie_adapter_timeout_total`
- `tie_verdict_total`
- `tie_cache_hit_ratio`
- `tie_async_queue_depth`
- `tie_webhook_delivery_total`
- `tie_signature_verification_fail_total`

---

## Error Taxonomy, Recovery, and Client Semantics

TIE needs a centralized error language so operators, SDKs, and integrations behave consistently.

### Error classes

**Client and contract errors**
- `invalid_request`
- `unsupported_subject_type`
- `payload_too_large`
- `invalid_policy_profile`
- `unauthorized`
- `forbidden`
- `rate_limited`

**Validation and evidence errors**
- `registry_conflict`
- `truth_not_found`
- `adapter_inconclusive`
- `evidence_missing`
- `signature_invalid`
- `policy_violation`

**Platform and dependency errors**
- `adapter_timeout`
- `dependency_unavailable`
- `queue_saturated`
- `storage_unavailable`
- `internal_error`

### Canonical response shape for errors

Every non-success response should include:
- `error_code`
- `error_class`
- `message`
- `retryable`
- `suggested_action`
- `trace_id`
- `verdict_id` if created
- `details` with machine-readable fields safe for clients

### Retry semantics

**Safe to retry automatically**
- `adapter_timeout` when request is idempotent and budget remains
- `dependency_unavailable` for transient integrations
- `queue_saturated` with backoff
- webhook delivery failures with signed idempotency key

**Do not auto-retry**
- `invalid_request`
- `forbidden`
- `policy_violation`
- `signature_invalid`
- `unsupported_subject_type`

**Retry policy**
- exponential backoff with jitter
- bounded retry count
- idempotency key required for externally visible actions
- no retries across changed truth snapshots unless explicitly requested

### User-facing message policy

User-facing errors should be short, actionable, and non-leaky.

Examples:
- `invalid_request`: "The validation request is missing required fields."
- `registry_conflict`: "Validation could not complete because the configured source of truth contains unresolved conflicts."
- `adapter_timeout`: "A required verifier exceeded its time budget."
- `policy_violation`: "The request failed a mandatory policy check."

Detailed internals should remain in audit logs and operator traces, not public client messages.

### Recovery patterns

- fall back from Tier 3 to Tier 2 only when policy explicitly allows provisional decisions
- preserve partial evidence from successful adapters
- quarantine inconclusive critical requests rather than silently passing
- emit a Kaizen event for every timeout, inconclusive result, and conflict-driven downgrade
- support operator replay of a validation request against the same truth snapshot for forensics

---

## Deployment, Configuration, and CI/CD Strategy

The architecture should specify not just where the service runs, but how safe change reaches production.

### Configuration management

Use layered configuration with explicit precedence:
1. compiled defaults
2. environment profile file such as `advisory.toml` or `full-fail-closed.toml`
3. environment variables
4. secret injection
5. startup overrides for one-off maintenance operations

### Helm and environment overrides

The Helm chart should expose, at minimum:
- image tag and digest pinning
- enforcement profile
- registry backend selection
- adapter enable/disable flags
- per-adapter timeout budgets
- worker pool sizes
- webhook signing keys reference
- resource requests and limits
- ingress/TLS settings
- observability endpoints
- persistent volume settings

Keep separate values files for:
- local development
- staging
- production
- high-security production

### Release strategy

**Preferred default**
- blue-green for registry-affecting or policy-affecting releases
- canary for stateless API and adapter changes
- one-click rollback to prior image plus prior signed registry snapshot manifest

### CI gates before merge

- format, lint, and test suites pass
- OpenAPI contract validation
- registry migration dry run
- signature verification of seeded truth data
- benchmark corpus regression check
- policy profile linting
- SAST and dependency vulnerability scan
- reproducible build metadata generation

### CD gates before production

- deploy to staging with production-like truth snapshot
- smoke-test all enabled adapters
- verify signed verdict issuance
- run synthetic hot-path latency probes
- confirm dashboard and alert readiness
- require explicit approval for signer key or registry schema changes

### Post-deploy checks

- compare p95 and timeout rates to baseline
- compare verdict distribution to prior release
- verify queue depth and webhook retry behavior
- verify no signature validation regressions
- hold canary before full rollout if any metric exceeds threshold

---

## Multi-Tenancy Isolation Plan

v1 launches single-tenant, but the data model and service boundaries should support hardening into multi-tenant deployment without redesign.

### Tenant model from day one

Every request, verdict, audit event, registry record, and cache namespace should reserve:
- `tenant_id`
- `environment_id`
- `policy_profile_id`

For single-tenant v1, these may default to a fixed internal value, but they should still exist in schemas and cache keys.

### Supported isolation levels

**Level 0: single tenant**
- one deployment
- one registry
- one signer domain

**Level 1: logical isolation**
- shared service
- tenant-scoped row filtering
- tenant-scoped cache keys
- tenant-specific policy profiles and signer keys

**Level 2: schema or database isolation**
- separate schema or database per tenant
- tenant-scoped backup and restore
- stronger operational blast-radius control

**Level 3: deployment isolation**
- separate service instances and storage per tenant
- dedicated encryption domains
- per-tenant worker pools and ingress

### Recommended future guarantees

When multi-tenant mode is enabled, TIE should guarantee:
- tenant-scoped authorization on every read and write
- no cross-tenant cache reuse
- per-tenant encryption keys for secrets and optionally stored evidence
- tenant-specific audit export
- row-level security or schema isolation depending on deployment tier
- tenant-specific rate limits and quota controls

### Migration path

- start with tenant fields present in schema
- introduce tenant-aware repository interfaces before enabling shared tenancy
- move from SQLite to Postgres before logical multi-tenancy
- enable row-level security for shared Postgres mode
- offer dedicated schema or dedicated deployment for higher-regulation customers

## Microservices Breakdown

### Recommended v1: modular monolith

For the first commercial-grade implementation, TIE should launch as a **modular monolith** with clear package boundaries, not as distributed microservices.

**Why**
- Lower latency for validation hot paths
- Fewer failure modes during early productization
- Easier debugging of verifier and registry interactions
- Faster iteration for policy and adapter contracts
- Lower operational cost for a single-tenant launch

### Logical service boundaries

Even inside a monolith, the system should preserve these extraction-ready boundaries:

1. **Gateway Boundary**
   - authentication
   - rate limiting
   - request normalization
   - trace context

2. **Validation Core Boundary**
   - routing
   - policy selection
   - verdict assembly

3. **Registry Boundary**
   - truth reads/writes
   - signature verification
   - versioning

4. **Adapter Boundary**
   - code verification
   - fact verification
   - action verification

5. **Kaizen Boundary**
   - learning loop
   - clustering
   - proposal generation

### Future extraction options

If scale or isolation requires it later, extract in this order:

1. Verifier worker pool
2. Registry service
3. Kaizen analytics worker
4. External gateway

---

## Validation Request Data Flow

```text
Client / Hook / Node / CLI
    │
    │ 1. Submit validation request
    ▼
API Gateway
    │
    │ 2. Authenticate + normalize + assign trace ID
    ▼
TIE Service Core
    │
    │ 3. Classify request: code / fact / action / mixed
    │ 4. Load enforcement profile
    │ 5. Query registry for scoped ground truth
    ▼
Ground Truth Registry
    │
    │ 6. Return relevant truth packs, policies, signatures, versions
    ▼
TIE Service Core
    │
    │ 7. Dispatch request + truth context to matching verifier adapters
    ├─────────────────────────────┬─────────────────────────────┬────────────────────────────┐
    ▼                             ▼                             ▼                            │
Code Verifier                Fact Verifier                 Action Verifier                  │
    │                             │                             │                            │
    │ 8a. Evaluate code           │ 8b. Evaluate claims         │ 8c. Evaluate action        │
    │     correctness/safety      │     provenance/consistency  │     policy/preconditions   │
    ▼                             ▼                             ▼                            │
Adapter verdicts + evidence + scores + timings
    └─────────────────────────────┴─────────────────────────────┴────────────────────────────┘
                                      │
                                      │ 9. Aggregate evidence
                                      │ 10. Apply policy and enforcement mode
                                      ▼
                               Decision Layer
                                      │
                                      │ 11. Emit pass/warn/fail/etc.
                                      │ 12. Persist audit + kaizen events
                                      ▼
                                Validation Response
                                      │
                                      ├── return to caller
                                      ├── optional webhook callback
                                      └── optional memory/report storage in integrations
```

### Request shape expectations

A validation request should support:
- `subject_type`: code, fact, action, or mixed
- `subject_payload`: content or reference under evaluation
- `context`: metadata, environment, workflow context, model info
- `policy_profile`: advisory, critical-fail-closed, full-fail-closed
- `truth_scope`: namespaces, record classes, registry filters
- `callback`: optional webhook destination
- `requested_verifiers`: explicit adapter selection override

### Response shape expectations

A validation response should include:
- normalized verdict
- per-adapter findings
- evidence bundle references
- confidence and severity
- timings and budgets
- trace ID and verdict ID
- recommended next action
- truth snapshot ID
- cache metadata
- signed verdict token when enabled

---

## Kaizen Loop Diagram

```text
              ┌──────────────────────────────────────────────────┐
              │            Live Validation Traffic              │
              └──────────────────────┬───────────────────────────┘
                                     │
                                     │ validation failures / warnings / inconclusives
                                     ▼
                      ┌──────────────────────────────────┐
                      │      Kaizen Event Collector      │
                      │ timings / errors / evidence      │
                      └────────────────┬─────────────────┘
                                       │
                                       │ normalize + cluster
                                       ▼
                      ┌──────────────────────────────────┐
                      │     Pattern & Drift Analyzer     │
                      │ false neg / false pos / gaps     │
                      └────────────────┬─────────────────┘
                                       │
                                       │ generate candidate improvements
                                       ▼
                      ┌──────────────────────────────────┐
                      │   Rule / Benchmark Proposal Set  │
                      │ policies / tests / truth packs   │
                      └────────────────┬─────────────────┘
                                       │
                                       │ human review or signed automation policy
                                       ▼
                      ┌──────────────────────────────────┐
                      │ Approved Improvement Artifacts   │
                      │ versioned + signed               │
                      └────────────────┬─────────────────┘
                                       │
                                       │ update registry / policies / benchmark corpus
                                       ▼
                      ┌──────────────────────────────────┐
                      │   Ground Truth Registry Update   │
                      └────────────────┬─────────────────┘
                                       │
                                       │ used by next validation cycle
                                       ▼
              ┌──────────────────────────────────────────────────┐
              │        Improved Future Validation Behavior       │
              └──────────────────────────────────────────────────┘
```

### Kaizen operating principles

- Every failure should be explainable
- Every recurring failure pattern should become a candidate benchmark
- Every policy change should be versioned and attributable
- Improvement artifacts should be signed before entering the trusted registry
- Kaizen must improve precision without silently reducing recall

---

## Integration Surfaces

### n8n Node
- Wraps TIE validation as a workflow step
- Supports pre-action validation, post-generation review, and branch-on-verdict
- Can store structured validation reports in workflow context

### ZeroClaw Hook
- Intercepts LLM outputs before release or execution
- Calls TIE on every relevant output or action proposal
- Stores validation reports for future memory and audit use
- Can enforce block, retry, or rewrite loops

### CLI
- Provides local developer validation
- Supports file, stdin, JSON payload, and batch modes
- Useful for CI, debugging, and registry inspection

### Webhooks
- Support asynchronous callback delivery
- Useful for long-running verifications or workflow continuation
- Require signing and replay protection

### OpenClaw / common agent adapters
- Shared adapter contract for agent frameworks
- Enables TIE as a universal validation boundary instead of a point integration

---

## Commercial Readiness Notes

### What is production-sensible already
- Clean deployment profile model
- Monolith-first with extraction seams
- Registry provenance and signing support
- Low-latency path separation from Kaizen learning loop
- Multi-integration ingress model
- Progressive verification tiers
- explicit cache invalidation model
- signed verdict path for downstream enforcement

### What should be enforced before GA
- registry bootstrap governance workflow
- conflict resolution policy enabled by default
- adapter SLOs and timeout budgets
- authN/authZ and signer key rotation
- backup/restore drills and restore verification
- runbooks for fail-open versus fail-closed decisions
- benchmark corpus with adversarial test cases
- alerting on timeout spikes, cache misses, and registry drift

---

## Recommended Foundation Decision

**Core language:** Rust  
**Architecture style:** Modular monolith  
**Ingress:** HTTP first, gRPC-ready boundary  
**Registry:** SQLite first with repository abstraction and signed snapshots  
**Verification style:** Progressive multi-adapter orchestration with deployment-configurable enforcement  
**Learning loop:** asynchronous Kaizen engine with signed improvement artifacts

This architecture gives TIE a fast path to a commercial-quality v1 while preserving clean growth paths into multi-tenant, distributed, and stricter-verification deployments.
