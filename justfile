alias b := build

[private]
default:
  @just --list

build-package PACKAGE *ARGS:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo build -p {{PACKAGE}} {{ARGS}}

build *ARGS:
  just build-package server {{ARGS}}
  just build-package cli {{ARGS}}
  just build-package spa --target wasm32-unknown-unknown {{ARGS}}

clippy-package PACKAGE *ARGS="--locked":
  cargo clippy -p {{PACKAGE}} {{ARGS}}

clippy *ARGS="--locked":
  just clippy-package server {{ARGS}}
  just clippy-package cli {{ARGS}}
  just clippy-package spa --target wasm32-unknown-unknown {{ARGS}}

build-container:
  #!/usr/bin/env bash
  set -euo pipefail
  nix build ".#containerImage"
  IMAGE_TAG=$(docker load < result | awk '{ print $3 }')
  docker tag "$IMAGE_TAG" "bookmark-hub:latest"

run-spa:
  #!/usr/bin/env bash
  set -euo pipefail
  pushd spa
  trunk serve
  popd

run-server:
  #!/usr/bin/env sh
  DATA_DIR="/tmp/bookmark-hub-datadir"
  if [[ ! -d "$DATA_DIR" ]]; then
    mkdir -p "$DATA_DIR"
  fi
  RUST_BACKTRACE=full RUST_LOG=info,server=debug cargo run --bin server -- \
    --hmac-key secret \
    --pg-host localhost \
    --pg-port 5432 \
    --pg-user main \
    --pg-password main \
    --pg-database main \
    --pg-max-connections 5 \
    --readability-url "http://localhost:3001" \
    --data-dir "$DATA_DIR" \
    --ollama-url "http://localhost:11434" \
    --ollama-text-model "gemma3:4b"

run-cli *ARGS:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo run --bin cli {{ARGS}}

test-integration: build
  #!/usr/bin/env bash
  set -euo pipefail
  RUST_BACKTRACE=1 RUST_LOG=info cargo test -p server --features integration-tests -- --nocapture --test-threads=1
