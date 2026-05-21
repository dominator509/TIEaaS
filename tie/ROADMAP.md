# TIE Delivery Roadmap

## Goal
Build TIE into a production-ready validation and trust-gating platform for code, factual claims, and actions, with strong integration surfaces for HTTP, webhooks, n8n, ZeroClaw, CLI, and future OpenClaw-compatible adapters.

## Current State
The repository is scaffolded and architecturally defined. The main gaps are build verification, refactoring generated code into a compiling Rust workspace, and hardening operational behavior under real tests.

## Phase 0 — Foundation and Architecture
Status: Complete

Deliverables:
- Commercial-grade architecture document
- Core repo structure
- Initial Cargo manifest
- Main service scaffold
- OpenAPI spec
- n8n node scaffold
- ZeroClaw hook scaffold
- Test suite skeletons
- Environment/bootstrap files
- Ops docs and integration docs
- Codex handoff notes

Exit criteria:
- Architecture approved
- Repo structure accepted

## Phase 1 — Build Stabilization
Status: Next critical phase

Objectives:
- Make the Rust service compile cleanly
- Split single-binary logic into reusable modules
- Wire config, migrations, telemetry, and server bootstrap together
- Validate Docker build and CI pipeline

Tasks:
1. Run `cargo check --workspace`
2. Resolve dependency/API mismatches
3. Replace placeholder/scaffold sections in `src/main.rs`
4. Move logic into `src/lib.rs`, `src/bootstrap.rs`, `src/app_state.rs`, and `src/telemetry.rs`
5. Ensure migrations run on startup or via CLI
6. Make `Dockerfile` and `docker-compose.yml` boot the service successfully
7. Make smoke tests pass

Exit criteria:
- `cargo check --workspace` passes
- `cargo test --workspace` passes or has clearly documented skips
- Docker image builds
- Service starts and responds to `/healthz`, `/readyz`, and `/v1/validate`

## Phase 2 — Registry and Policy Hardening
Objectives:
- Harden Ground Truth Registry behavior
- Implement record signing and verification
- Implement contradiction resolution and provenance rules
- Add policy profile loading and effective-policy introspection

Tasks:
- Finalize SQLite schema and indexes
- Add CRUD validation and error taxonomy
- Implement Ed25519 signing and verification
- Add hot-path cache invalidation logic
- Add policy CRUD and effective policy resolution

Exit criteria:
- Registry CRUD is reliable
- Signatures verified on read/write paths where required
- Policy modes work: advisory, critical-fail-closed, full-fail-closed

## Phase 3 — Verifier Adapters
Objectives:
- Deliver fast-path and slow-path validation
- Establish deterministic adapter contracts
- Add timeout budgets and fallback rules

Tasks:
- Implement code verifier adapter contract
- Implement fact verifier evidence and citation checks
- Implement action verifier policy/runbook checks
- Add adapter-level cache keys and TTLs
- Add async escalation path for expensive checks

Exit criteria:
- All three validator classes work end-to-end
- Critical checks block correctly per deployment policy
- Latency budgets are measured and reported

## Phase 4 — Integration Surfaces
Objectives:
- Make integrations production-usable

Tasks:
- Validate and publish n8n node package
- Harden ZeroClaw hook with retries, logging, and persistence checks
- Add webhook signature verification and delivery retry behavior
- Build out CLI UX and packaging
- Prepare future gRPC/OpenClaw seam

Exit criteria:
- n8n node works in live n8n
- ZeroClaw hook passes integration tests
- CLI and webhooks are documented and tested

## Phase 5 — Kaizen Engine and Feedback Loops
Objectives:
- Turn failures into actionable improvement signals

Tasks:
- Normalize validation errors into a common taxonomy
- Cluster recurring failures
- Propose policy or registry updates
- Add metrics and dashboards for drift/error trends

Exit criteria:
- Kaizen pipeline persists and reports useful trends
- Error patterns are visible in docs or dashboards

## Phase 6 — Production Readiness
Objectives:
- Make the system operator-ready and supportable

Tasks:
- Add complete observability dashboards and alerts
- Validate backup/restore workflow
- Add canary or blue-green deployment guidance
- Add security review checklist
- Add load/performance tests
- Finalize operational runbooks

Exit criteria:
- SLOs defined and measurable
- Alerting and dashboards documented
- Backup/restore tested
- Security and deployment procedures documented

## Recommended Execution Order
1. Build stabilization
2. Docker/CI validation
3. Registry/policy completion
4. Verifier completion
5. Integration hardening
6. Kaizen implementation
7. Production readiness and release packaging

## Definition of Done
- Code compiles and tests pass
- Service boots under Docker and locally
- Core validation flows work for code, facts, and actions
- Registry and policy storage are durable and signed where required
- Integration surfaces are usable and documented
- Observability, security, and deployment docs are complete
