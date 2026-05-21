# TIE

TIE is a validation and gating service for code, facts, and actions.
It exposes an HTTP API, optional gRPC-ready service boundaries, a CLI, webhook callbacks, an n8n custom node, and a ZeroClaw interception hook.

## What is in this repository

- `src/main.rs` — current single-binary reference implementation
- `openapi.yaml` — API contract
- `config/` — deployment profiles
- `migrations/` — SQLite bootstrap and schema evolution
- `n8n-node-tie/` — n8n integration package
- `zeroclaw-hook/` — Python interception hook
- `tests/` — verifier, kaizen, and end-to-end tests
- `docs/` — operational and integration guides

## Quick start

### 1. Install prerequisites

- Rust via `rustup`
- SQLite development libraries
- Docker and Docker Compose if you want containerized local runs

### 2. Configure environment

Copy the example environment file:

```bash
cp .env.example .env
```

Set at minimum:

- `TIE_DATABASE_URL`
- `TIE_HTTP_BIND`
- `TIE_POLICY_MODE`
- `TIE_SIGNING_KEY_HEX` if you want signed registry records and verdict artifacts

### 3. Run locally with Cargo

```bash
cargo check
cargo run -- serve
```

Health checks:

```bash
curl http://127.0.0.1:8080/healthz
curl http://127.0.0.1:8080/readyz
```

### 4. Run with Docker Compose

```bash
docker compose up --build
```

### 5. Seed a registry record

```bash
./scripts/seed_registry.sh
```

### 6. Smoke test validation

```bash
./scripts/smoke_test.sh
```

## Deployment profiles

TIE supports three enforcement profiles:

- `advisory`
- `critical-fail-closed`
- `full-fail-closed`

Example:

```bash
cargo run -- serve --policy-mode critical-fail-closed
```

Or via environment:

```bash
export TIE_POLICY_MODE=critical-fail-closed
```

## Database

The current implementation bootstraps SQLite tables at service start. The SQL migration files in `migrations/` are the canonical schema history for CI/CD, backup validation, and future migration to an external database service.

## API surface

Main endpoints:

- `POST /v1/validate`
- `GET /v1/registry/records`
- `POST /v1/registry/records`
- `GET /v1/registry/records/{id}`
- `PUT /v1/registry/records/{id}`
- `DELETE /v1/registry/records/{id}`
- `GET /healthz`
- `GET /readyz`

See `openapi.yaml` and `docs/api_usage.md` for examples.

## Operational notes

- Start single-tenant, but keep tenant fields and deployment seams ready for later isolation expansion.
- Use `critical-fail-closed` as the default production posture unless the deployment explicitly requires advisory-only behavior.
- Keep long-running or solver-heavy verifier paths behind budget enforcement and async escalation.

## Current implementation status

This repository is architected to commercial standards, but some generated code has not yet been compiler-validated in this execution environment because the Rust toolchain was not installed here at authoring time. Use the included CI workflow and local toolchain to validate and tighten the code in the next pass.
