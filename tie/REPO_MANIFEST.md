# TIE Repository Manifest

This archive contains the current TIE project scaffold, including architecture, bootstrap, code, tests, integrations, and handoff materials.

## Top-level key files
- `ARCHITECTURE.md` — approved architecture and production design
- `ROADMAP.md` — delivery roadmap and execution phases
- `CODEX_HANDOFF.md` — Codex-oriented handoff
- `GOOGLE_JULES_HANDOFF_PROMPT.md` — ready-to-use prompt for Google Jules
- `README.md` — quickstart and repo overview
- `Cargo.toml` — Rust dependencies and feature flags
- `openapi.yaml` — API contract
- `Dockerfile` / `docker-compose.yml` — container runtime and local orchestration

## Source areas
- `src/` — Rust service code
- `config/` — deployment profiles
- `migrations/` — SQLite schema migrations
- `tests/` — Rust and Python tests
- `docs/` — operator and integration documentation
- `n8n-node-tie/` — n8n custom node
- `zeroclaw-hook/` — Python interception hook
- `webhooks/` — webhook examples and schemas
- `scripts/` — helper scripts

## Archive intent
This snapshot is intended as a strong handoff bundle for an implementation-focused coding agent or engineer who will compile, run, verify, and harden the repo.
