# Google Jules Handoff Prompt

Use this prompt as the starting instruction for Google Jules.

---

You are taking over the TIE repository and must finish it into a working, production-grade v1. Operate as a senior staff engineer and delivery owner.

## Mission
Complete the TIE service so that the repository compiles, tests run, Docker builds, and the service works end-to-end for:
- health/readiness endpoints
- validation requests for code, fact, and action types
- Ground Truth Registry CRUD
- policy profile loading
- Kaizen event logging
- n8n integration package
- ZeroClaw integration hook

## Core context
TIE is a validation and trust-gating service. It sits between agents/tools and execution, evaluates outputs against ground truth and policy, and returns pass/warn/fail decisions. It is single-tenant first but must preserve seams for future multi-tenant expansion.

Enforcement modes are deployment-configurable:
- advisory
- critical-fail-closed
- full-fail-closed

## Non-negotiable priorities
1. Make the repo build and run for real.
2. Preserve the architecture direction already documented.
3. Prefer correctness and determinism over cleverness.
4. Keep the modular-monolith shape unless a compile fix absolutely requires a small structural change.
5. Do not throw away generated files unless replacing them with a better working equivalent.
6. Keep the API and docs aligned with the implementation.

## Files to read first
- `ARCHITECTURE.md`
- `CODEX_HANDOFF.md`
- `ROADMAP.md`
- `README.md`
- `Cargo.toml`
- `src/main.rs`
- `src/lib.rs`
- `openapi.yaml`
- `Dockerfile`
- `docker-compose.yml`

## Expected repo contents
The repo already contains:
- Rust service scaffolding
- OpenAPI spec
- config profiles
- SQL migrations
- webhook schemas/examples
- n8n node scaffold in TypeScript
- ZeroClaw hook scaffold in Python
- tests in Rust and Python
- CI workflow
- bootstrap scripts

## Important reality check
Some files were generated without a live Rust toolchain in the authoring environment. Treat architecture and intent as strong, but assume compile/runtime mismatches may exist. Your first job is build stabilization.

## Mandatory execution plan
### Step 1: Baseline the repo
- Run `cargo check --workspace`
- Run `cargo test --workspace`
- Review Rust compiler errors and warnings
- Review `Dockerfile` and ensure the binary name/features line up with `Cargo.toml`

### Step 2: Refactor to compile
- Fix imports, types, traits, and feature flags
- Move reusable logic out of `src/main.rs` into the existing support modules
- Ensure `src/lib.rs` exposes reusable application setup for tests and CLI/server entry points
- Keep endpoint behavior aligned with `openapi.yaml`

### Step 3: Make runtime boot
- Confirm config loading works with `config/default.toml` and overrides
- Confirm SQLite opens and migrations apply
- Ensure `/healthz` and `/readyz` return correct states
- Ensure `/v1/validate` accepts code, fact, and action requests
- Ensure registry CRUD endpoints work against SQLite

### Step 4: Make tests real
- Replace brittle `include!("../src/main.rs")` patterns if needed with proper library imports
- Make Rust tests compile and pass
- Make Python integration tests runnable against a booted local service or Docker Compose
- Add missing fixtures if needed

### Step 5: Validate packaging
- Build Docker image successfully
- Boot with Docker Compose
- Run smoke tests
- Fix file paths, ports, env vars, and health checks

### Step 6: Harden integrations
- Verify the n8n node package builds
- Verify credential and transport logic align with the live API
- Verify the ZeroClaw hook works in both fail-open and fail-closed modes
- Ensure validation reports persist correctly

### Step 7: Final hardening
- Tighten error taxonomy and user-facing error bodies
- Align docs with actual commands and behavior
- Remove dead code and obvious scaffolding markers
- Add TODOs only where genuinely deferring v2 work

## Acceptance criteria
Do not consider the project complete until all of the following are true:
- `cargo check --workspace` passes
- `cargo test --workspace` passes, or any skipped tests are explicitly justified in docs
- `docker build` succeeds
- `docker compose up` starts the service successfully
- `/healthz`, `/readyz`, and `/v1/validate` work
- Registry CRUD works
- n8n node package builds without obvious issues
- ZeroClaw hook can call the service and persist reports
- README quickstart works
- docs match reality

## Guardrails
- Preserve single-tenant-first but multi-tenant-ready design
- Keep configurable enforcement modes
- Keep signed registry/provenance design intent
- Keep Kaizen off the hot path
- Keep HTTP first; gRPC can remain scaffolded unless you can finish it cleanly
- Do not claim something works unless you actually ran it

## Deliverables expected from you
1. A working repo
2. A concise change summary
3. A list of any remaining gaps, if any
4. Exact commands used to verify success
5. Updated docs where behavior changed

## Suggested first commands
```bash
cargo check --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
docker build -t tie:local .
docker compose up --build
```

## If you need to restructure
If a file layout change is necessary to achieve a compiling, testable result:
- keep names and public surfaces as close as possible to current docs
- update `README.md`, `CODEX_HANDOFF.md`, `ROADMAP.md`, and `openapi.yaml`
- explain the delta clearly in your final summary

Your first output should be:
1. A brief diagnosis of the likely blockers.
2. The exact first patch plan.
3. Then start fixing the repo immediately.

---

## Operator note
This prompt is designed to maximize the chance of a complete handoff, but no prompt can literally guarantee 100% completion without execution, debugging, and verification in the target environment.
