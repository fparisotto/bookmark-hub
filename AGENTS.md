# AGENTS.md

Rust workspace (`server`, `cli`, `spa`, `shared`). Nix flake provides the toolchain and is what CI uses; `use flake` is wired via `.envrc`/direnv.

## Commands
- `just build` — builds `server`, `cli`, and `spa` (the latter with `--target wasm32-unknown-unknown`; the WASM target must be installed).
- `just clippy` — workspace clippy with `--locked` per package. CI runs `nix develop .# --command just clippy` and treats warnings as errors (`--deny warnings` in flake checks). Fix clippy before pushing.
- `cargo fmt --all` — formatting; `.rustfmt.toml` uses unstable options (`imports_granularity = "Module"`, `group_imports`, `edition = "2024"`), so **nightly rustfmt is required**. The Nix devShell puts nightly `rustfmt` on PATH; outside Nix, install it manually or stable rustfmt will silently skip those rules.
- `just run-server` — runs the API against a local Postgres (`localhost:5432`, user/db `main`/`main`) and Ollama at `localhost:11434`. Hardcoded dev creds; override via env vars (`PG_*`, `HMAC_KEY`, `LLM_*`) if needed.
- `just run-spa` — `trunk serve` on `:8080`; `spa/Trunk.toml` proxies `/api/v1/` and `/static/` to `localhost:3000`, so run the server first.
- `just run-cli -- …` — CLI; `login` is required before other commands.

## Testing
- Integration tests live in `server/tests/integration_db_<area>.rs` and are **feature-gated**: they compile only with `--features integration-tests` and `#![cfg(feature = "integration-tests")]` at the top of each file.
- Run them with `just test-integration` (builds first). **Must use `--test-threads=1`** — already set in the justfile; do not parallelize.
- Tests spin up Postgres via `testcontainers` (see `server/tests/common/`), so **Docker is required** but no manual DB setup. Tests create a unique DB per run.
- `test.hurl` / `bootstrap.hurl` are Hurl smoke tests against a **running** server. `hurl --verbose --test test.hurl`.
- CI: `nix flake check -L .#` (build + clippy + fmt) plus a separate `integration-tests` job running the gated cargo test command.

## Architecture notes
- `server`: Axum API + background daemons. DB layer in `server/src/db/`, handlers in `server/src/endpoints/`. Uses `deadpool-postgres`, `pgvector`, `rig-core` for LLM providers, `headless_chrome`/Browserless for content extraction.
- `spa`: Yew WASM app built with Trunk; output goes to `spa/dist`, which the server serves when `SPA_DIST` is set (flake sets it at build time).
- `shared`: types shared between crates; depend on it via `path = "../shared"`.
- SQL migrations are numbered files in `server/schema/` (currently `1_*` through `9_*`). Add new ones with the next sequential prefix; do not edit applied migrations.
- AI features (tagging, summarization, embeddings, RAG) are **disabled unless `LLM_TEXT_MODEL` is set**. Provider is `ollama` by default; cloud providers need their respective `*_API_KEY`. See README for the full env matrix.

## Conventions
- Conventional Commit prefixes (`fix:`, `chore:`, `feat:` …) with imperative summaries.
- `snake_case` for modules/functions, `PascalCase` for types and Yew components.
- Keep `README.md` env defaults and `justfile` flags aligned when changing config behavior.
- Do not commit real API keys or DB credentials.

## Container
- `just build-container` builds the Nix Docker image and tags it `bookmark-hub:latest`. `docker-compose.yml` runs Postgres + Browserless Chrome + the server on `:3000`; `docker-compose.host-ollama.yml` uses host networking and `pgvector/pgvector:pg17` for embeddings.