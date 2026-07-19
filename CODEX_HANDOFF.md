# CODEX HANDOFF

## Project
TIE is a validation and gating service for **code**, **facts**, and **actions**.
It is currently scaffolded as a **single-binary Rust service** with integration assets for:
- HTTP API
- gRPC-ready seam
- CLI
- webhooks
- n8n custom node
- ZeroClaw hook

The repo is architecturally complete for Phase 0/1 scaffolding, but **not yet build-verified** in a live Rust toolchain.

---

## Current status

### Completed and present in repo
- `ARCHITECTURE.md` — approved architecture baseline
- `Cargo.toml` — Rust dependency set and feature flags
- `src/main.rs` — single-binary reference implementation
- `src/lib.rs`, `src/app_state.rs`, `src/bootstrap.rs`, `src/telemetry.rs` — extraction seams for modularization
- `openapi.yaml` — API contract
- `config/*.toml` — deployment profiles
- `migrations/*.sql` — SQLite schema history
- `tests/*.rs`, `tests/integration_tests.py` — initial test suites
- `n8n-node-tie/` — custom node package scaffold
- `zeroclaw-hook/` — Python interception hook scaffold
- `webhooks/` — payload examples and JSON Schemas
- `Dockerfile`, `docker-compose.yml`, `.devcontainer/`, `.github/workflows/ci.yml` — environment and CI scaffolding
- `README.md`, `docs/*.md`, scripts — operator/developer docs and helper scripts

### Important caveat
This repo was generated **without running `cargo check`, `cargo test`, or a live n8n/ZeroClaw build loop in the authoring environment**.
Assume there are compile issues, import mismatches, feature mismatches, and some incomplete seams until validated.

---

## Highest-priority objective for Codex
Turn the generated architecture into a **buildable, testable, bootable repository** with the least structural churn possible.

### Definition of done for handoff phase
1. `cargo check --all-features` passes
2. `cargo test` passes or failing tests are explicitly quarantined with TODOs
3. service boots locally with SQLite
4. `/healthz`, `/readyz`, `/v1/validate`, and registry CRUD work end-to-end
5. `openapi.yaml` matches the actual handlers
6. CI passes
7. Docker build succeeds
8. n8n node TypeScript package builds
9. ZeroClaw hook works against local TIE instance

---

## Recommended execution order

### 1) Stabilize Rust build first
Run:
```bash
cargo check
cargo check --all-features
cargo test
```

Expected work:
- fix dependency/version mismatches in `Cargo.toml`
- fix imports, derives, trait bounds, and feature-gated code in `src/main.rs`
- fix `utoipa`/Swagger annotations if broken
- fix `sqlx` usage and any query/row mapping issues
- fix `ed25519-dalek` API usage if crate version and code diverge
- fix `moka` cache API usage if signatures are wrong
- fix `clap` subcommand parsing and env flag issues

### 2) Make `src/main.rs` boot reliably
Target outcome:
- `cargo run -- serve` starts
- DB bootstrap succeeds against `sqlite://tie.db`
- service responds on configured bind address

Check:
```bash
curl http://127.0.0.1:8080/healthz
curl http://127.0.0.1:8080/readyz
```

### 3) Validate registry CRUD
Endpoints to verify:
- `GET /v1/registry/records`
- `POST /v1/registry/records`
- `GET /v1/registry/records/{id}`
- `PUT /v1/registry/records/{id}`
- `DELETE /v1/registry/records/{id}`

Make sure:
- migrations match actual row model
- signatures/digests are computed deterministically
- cache invalidation happens on write/update/delete
- version increments behave consistently

### 4) Validate `/v1/validate`
Confirm request routing for:
- `code`
- `fact`
- `action`

Make sure:
- policy mode changes final verdict behavior correctly
- adapter timeout budgets work
- canonical error envelopes are returned
- response schema matches `openapi.yaml`
- validation cache key is stable and correct

### 5) Reconcile modularization seam
The repo currently has both:
- large single-binary `src/main.rs`
- early extraction files in `src/lib.rs`, `src/app_state.rs`, `src/bootstrap.rs`, `src/telemetry.rs`

Codex should decide one of two paths:

#### Option A — fast stabilization
Keep `main.rs` as source of truth until build is green.
Only use the other files if they are directly imported and needed.

#### Option B — controlled refactor after green build
Once build passes, move shared types and boot logic into `lib.rs` and thin out `main.rs`.

Recommended: **Option A first**.

### 6) Align migrations with runtime bootstrap
Make sure one schema path is authoritative.
Recommended:
- treat `migrations/*.sql` as canonical
- make runtime bootstrap idempotent or switch to `sqlx::migrate!()`
- do not keep two divergent schema definitions

### 7) Tighten tests
Likely issues:
- Rust tests may rely on `include!("../src/main.rs")`, which is fragile
- integration tests may assume binary path or startup behavior incorrectly
- Python test may need retries or subprocess readiness logic

Recommended actions:
- move reusable logic into library functions
- make tests use helpers instead of including `main.rs`
- create ephemeral SQLite DB per test where possible

### 8) Validate non-Rust integrations
#### n8n node
Run install/build in `n8n-node-tie/`:
```bash
npm install
npm run build
```
Fix:
- package metadata
- credential schema
- Node API typings
- output item shape
- icon path/build config

#### ZeroClaw hook
Run basic validation against a local TIE server:
```bash
python -m py_compile zeroclaw-hook/*.py
```
Then execute a tiny smoke script that wraps a mock generation function.
Fix:
- local SQLite report store behavior
- HTTP request/timeout handling
- fail-open/fail-closed logic
- serialization consistency with TIE API

### 9) Validate Docker and CI
Run:
```bash
docker compose up --build
```
And ensure CI steps match actual repo structure and installed dependencies.

---

## Known likely problem areas

### Rust dependency / API drift
Most likely breakpoints:
- `actix-web`
- `utoipa` + `utoipa-swagger-ui`
- `sqlx`
- `ed25519-dalek`
- `z3`
- `tracing-opentelemetry`

Codex should prefer **minimal code changes** over large dependency downgrades unless a crate version is clearly incompatible.

### gRPC seam is scaffold only
`grpc` support is feature-gated and architecturally intended, but not expected to be fully implemented yet.
Do not block MVP stabilization on full gRPC.

### Verifier implementations are probably placeholder-grade
The repo includes logical adapter boundaries for code/fact/action validation, but the first goal is not advanced formal verification.
The first goal is:
- correct endpoint behavior
- deterministic verdict composition
- policy enforcement
- time budgets
- testability

### OpenAPI vs implementation drift
Expect drift between `openapi.yaml` and live handlers. Reconcile after the server boots.

---

## Concrete Codex task list

### Must do now
- [ ] install toolchain / run build
- [ ] make Rust compile
- [ ] make tests run
- [ ] make service boot
- [ ] verify health and readiness endpoints
- [ ] verify registry CRUD
- [ ] verify `/v1/validate`
- [ ] align schema/migrations/bootstrap
- [ ] ensure CI passes
- [ ] ensure Docker build passes

### Should do next
- [ ] thin `main.rs` by moving stable code into `lib.rs`
- [ ] replace fragile test patterns with helper-based tests
- [ ] add a real migration runner
- [ ] add structured error codes and response snapshots
- [ ] add request/response golden tests
- [ ] add benchmark smoke for hot path latency

### Nice to have after stabilization
- [ ] implement real gRPC service
- [ ] split verifier adapters into crates/workspace structure
- [ ] add signed verdict token verification library for clients
- [ ] add tenant isolation scaffolding beyond fields/config
- [ ] add persistent kaizen queue or async worker

---

## Suggested commands for Codex

### Setup
```bash
cp .env.example .env
cargo check
cargo check --all-features
cargo test
```

### Run service
```bash
cargo run -- serve
```

### Smoke test
```bash
./scripts/smoke_test.sh
./scripts/seed_registry.sh
```

### Docker
```bash
docker compose up --build
```

### n8n package
```bash
cd n8n-node-tie
npm install
npm run build
```

### Python hook
```bash
python -m py_compile zeroclaw-hook/*.py
python tests/integration_tests.py
```

---

## Suggested first Codex prompt
Use this as the first instruction to Codex:

> Stabilize this TIE repository into a buildable MVP with minimal architecture changes. Start by running `cargo check`, fix all compile errors in `Cargo.toml` and `src/main.rs`, then make `cargo test` pass. After the Rust service boots, verify `/healthz`, `/readyz`, `/v1/validate`, and registry CRUD against SQLite. Reconcile `openapi.yaml` with the implementation, then validate Docker, CI, the n8n node build, and the ZeroClaw hook. Prefer minimal edits and keep the approved architecture intact.

---

## Files Codex should treat as source of truth
- `ARCHITECTURE.md`
- `README.md`
- `openapi.yaml`
- `Cargo.toml`
- `src/main.rs`
- `migrations/*.sql`
- `config/*.toml`

If there is a conflict between architecture and current code, prefer the architecture unless the code change needed would be disproportionately large for MVP stabilization.

---

## Handoff note
This repo is in a good **design-complete / implementation-scaffolded** state, not yet a **compiler-verified production state**. The shortest path to value is a disciplined stabilization pass, not another architecture rewrite.
