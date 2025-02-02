alias b := build

[private]
default:
  @just --list

build_package PACKAGE *ARGS:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo build -p {{PACKAGE}} {{ARGS}}

build *ARGS:
  just build_package server {{ARGS}}
  just build_package spa --target wasm32-unknown-unknown {{ARGS}}

build_container:
  #!/usr/bin/env bash
  set -euo pipefail
  nix build ".#containerImage"
  IMAGE_TAG=$(docker load < result | awk '{ print $3 }')
  docker tag "$IMAGE_TAG" "bookmark-hub:latest"

clippy_package PACKAGE *ARGS="--locked":
  cargo clippy -p {{PACKAGE}} {{ARGS}}

clippy *ARGS="--locked":
  just clippy_package server {{ARGS}}
  just clippy_package spa --target wasm32-unknown-unknown {{ARGS}}

run-spa:
  #!/usr/bin/env bash
  set -euo pipefail
  pushd spa
  trunk serve
  popd
