#!/usr/bin/env bash
set -euo pipefail

export $(grep -v '^#' .env 2>/dev/null | xargs -r)

cargo check
cargo run -- serve
