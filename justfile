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
  just build-package spa --target wasm32-unknown-unknown {{ARGS}}

clippy-package PACKAGE *ARGS="--locked":
  cargo clippy -p {{PACKAGE}} {{ARGS}}

clippy *ARGS="--locked":
  just clippy-package server {{ARGS}}
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
