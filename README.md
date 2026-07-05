# Bookmark Hub

A self-hosted bookmark management application that helps you organize, search, and discover your saved links. Built with Rust, featuring AI-powered summarization and tagging to make your bookmarks more useful and discoverable.

## Features

- **Offline-First**: Store and manage bookmarks entirely on your own infrastructure
- **AI-Powered Organization**: Automatic tagging and summarization with multi-provider LLM support (Ollama, OpenAI, Anthropic, Gemini, OpenRouter)
- **RAG-Enhanced Search**: Intelligent search using Retrieval-Augmented Generation to find relevant bookmarks based on semantic similarity
- **Full-Text Search**: Search through bookmark titles, URLs, content, and AI-generated summaries
- **Tag Management**: Organize bookmarks with manual and AI-suggested tags
- **Content Extraction**: Automatically extract and store readable content from web pages
- **Modern Web Interface**: Responsive WebAssembly-based frontend built with Yew
- **REST API**: Complete API for programmatic access and integrations
- **MCP Server**: Expose bookmarks, search, tagging, and RAG to AI clients over the Model Context Protocol (Streamable HTTP transport, bearer-token auth)
- **CLI Tools**: Command-line interface for batch operations and automation

## How to Run

### Quick Start (Docker)

The easiest way to get started is using Docker Compose:

```bash
# Build the application container
$ just build-container

# Start the full stack (database + application)
$ docker compose up
```

This will start:
- PostgreSQL database on port 5432
- Browserless Chrome automation service on port 3001
- Bookmark Hub server on port 3000
- Web interface accessible at http://localhost:3000

#### Using with Host Ollama

If you already have Ollama running on your host machine, use the alternative compose file:

```bash
# Build the application container
$ just build-container

# Start with host Ollama integration
$ docker compose -f docker-compose.host-ollama.yml up
```

This configuration:
- Uses `network_mode: host` for direct access to host services
- Connects to Ollama running at `localhost:11434`
- Uses `pgvector/pgvector:pg17` for vector embedding storage
- Uses default Ollama models for AI features

### Development Setup

For local development with hot reloading:

#### Prerequisites
- Rust toolchain with `wasm32-unknown-unknown` target
- [Just](https://github.com/casey/just) task runner
- [Trunk](https://trunkrs.dev/) for WebAssembly builds
- PostgreSQL database
- Optional: [Ollama](https://ollama.ai/) for local AI features, or an API key for a cloud provider (OpenAI, Anthropic, Gemini, OpenRouter)

#### Running Components

1. **Start PostgreSQL database** (locally or via Docker)

2. **Start the server**:
   ```bash
   $ just run-server
   ```
   This starts the API server with development configuration.

3. **Start the frontend** (in a separate terminal):
   ```bash
   $ just run-spa
   ```
   This starts the development server at http://localhost:8080 with hot reloading.

#### Configuration

The server accepts configuration via environment variables:

```bash
# Database
export PG_HOST=localhost
export PG_PORT=5432
export PG_USER=your_user
export PG_PASSWORD=your_password
export PG_DATABASE=bookmark_hub

# Application
export HMAC_KEY=your_secret_key
export APP_DATA_DIR=/path/to/data

# Optional: Chrome Automation
export CHROME_HOST=localhost
export CHROME_PORT=3001
```

#### LLM Provider Configuration

AI features (tagging, summarization, embeddings, RAG) are disabled when `LLM_TEXT_MODEL` is not set. To enable them, configure a provider:

**Ollama (local, default):**
```bash
export LLM_PROVIDER=ollama
export OLLAMA_URL=http://localhost:11434
export LLM_TEXT_MODEL=qwen3.5:4b
export LLM_EMBEDDING_MODEL=qwen3-embedding:0.6b
# Optional override; auto-detected when omitted
export LLM_EMBEDDING_DIMENSION=1024
# Optional text chunk overrides; defaults stay conservative for local models
# export AI_TEXT_CHUNK_SIZE=1000
# export AI_TEXT_CHUNK_OVERLAP=100
```

**OpenAI:**
```bash
export LLM_PROVIDER=openai
export OPENAI_API_KEY=sk-...
export LLM_TEXT_MODEL=gpt-4o
export LLM_EMBEDDING_MODEL=text-embedding-3-small
export LLM_EMBEDDING_DIMENSION=1536
```

**Anthropic** (requires a separate embedding provider since Anthropic has no embedding API):
```bash
export LLM_PROVIDER=anthropic
export ANTHROPIC_API_KEY=sk-ant-...
export LLM_TEXT_MODEL=claude-sonnet-4-20250514
export LLM_EMBEDDING_PROVIDER=openai
export LLM_EMBEDDING_API_KEY=sk-...
export LLM_EMBEDDING_MODEL=text-embedding-3-small
export LLM_EMBEDDING_DIMENSION=1536
```

**Gemini:**
```bash
export LLM_PROVIDER=gemini
export GEMINI_API_KEY=AIza...
export LLM_TEXT_MODEL=gemini-2.5-flash
export LLM_EMBEDDING_MODEL=gemini-embedding-001
export LLM_EMBEDDING_DIMENSION=1536
# Default text chunking is larger for non-Ollama providers to reduce request count
# export AI_TEXT_CHUNK_SIZE=2000
# export AI_TEXT_CHUNK_OVERLAP=200
```

**OpenRouter:**
```bash
export LLM_PROVIDER=openrouter
export OPENROUTER_API_KEY=sk-or-...
export LLM_TEXT_MODEL=anthropic/claude-sonnet-4
export LLM_EMBEDDING_PROVIDER=openai
export LLM_EMBEDDING_API_KEY=sk-...
export LLM_EMBEDDING_MODEL=text-embedding-3-small
export LLM_EMBEDDING_DIMENSION=1536
```

You can mix providers — for example, use Anthropic for text and OpenAI for embeddings via `LLM_EMBEDDING_PROVIDER` and `LLM_EMBEDDING_API_KEY`.

Text chunk defaults are provider-aware when `AI_TEXT_CHUNK_*` is unset:

- `ollama` uses `1000` size and `100` overlap
- other text providers use `2000` size and `200` overlap

Embeddings use `AI_EMBED_CHUNK_SIZE=2000` and `AI_EMBED_CHUNK_OVERLAP=200` by default.

| Variable | Default | Description |
|---|---|---|
| `LLM_PROVIDER` | `ollama` | Text completion provider |
| `LLM_TEXT_MODEL` | _(none, disables AI)_ | Model for text tasks |
| `LLM_EMBEDDING_PROVIDER` | _(falls back to LLM_PROVIDER)_ | Embedding provider |
| `LLM_EMBEDDING_MODEL` | _(falls back to LLM_TEXT_MODEL)_ | Model for embeddings |
| `LLM_EMBEDDING_DIMENSION` | _(auto-detected)_ | Optional embedding vector dimension override |
| `LLM_EMBEDDING_API_KEY` | _(none)_ | API key for embedding provider if different |
| `LLM_REQUEST_TIMEOUT_SECS` | `120` | HTTP request timeout for LLM calls |
| `AI_TEXT_CHUNK_SIZE` | provider-aware | Text chunk size. Defaults to `1000` for `ollama`, `2000` otherwise |
| `AI_TEXT_CHUNK_OVERLAP` | provider-aware | Text chunk overlap. Defaults to `100` for `ollama`, `200` otherwise |
| `AI_EMBED_CHUNK_SIZE` | `2000` | Embedding chunk size |
| `AI_EMBED_CHUNK_OVERLAP` | `200` | Embedding chunk overlap |
| `AI_TEXT_CLAIM_WINDOW_SECS` | `1800` | Lease window for claimed text-AI work |
| `AI_EMBED_CLAIM_WINDOW_SECS` | `900` | Lease window for claimed embedding work |
| `LLM_MAX_IN_FLIGHT_TOTAL` | `4` | Shared concurrency cap across all LLM requests |
| `LLM_MAX_IN_FLIGHT_BACKGROUND` | `2` | Background concurrency cap reserved below the total cap |
| `LLM_TEXT_RPM_INTERACTIVE` | _(unset)_ | Optional interactive text request pacing |
| `LLM_TEXT_RPM_BACKGROUND` | _(unset)_ | Optional background text request pacing |
| `LLM_EMBED_RPM_INTERACTIVE` | _(unset)_ | Optional interactive embedding pacing |
| `LLM_EMBED_RPM_BACKGROUND` | _(unset)_ | Optional background embedding pacing |
| `LLM_RETRY_BASE_DELAY_MS` | `1000` | Base delay for transient LLM retries |
| `LLM_RETRY_MAX_DELAY_MS` | `30000` | Maximum delay for transient LLM retries |
| `OLLAMA_URL` | `http://localhost:11434` | Ollama base URL |
| `OPENAI_API_KEY` | _(none)_ | OpenAI API key |
| `ANTHROPIC_API_KEY` | _(none)_ | Anthropic API key |
| `GEMINI_API_KEY` | _(none)_ | Google Gemini API key |
| `OPENROUTER_API_KEY` | _(none)_ | OpenRouter API key |

### CLI Usage

```bash
# Login (required first)
$ just run-cli login --url http://localhost:3000 --username your_user --password your_password

# Add a single bookmark
$ just run-cli add --url https://example.com

# Add multiple bookmarks from a file (one URL per line)
$ just run-cli add-batch --file urls.txt
```

## Testing

Run end-to-end tests using [Hurl](https://hurl.dev/) (requires running application):

```bash
$ hurl --verbose --test test.hurl
```

## Architecture

- **Server**: Axum-based REST API with background processing daemons
- **Frontend**: Yew WebAssembly application for modern web experience  
- **Database**: PostgreSQL for reliable data storage with vector embeddings
- **AI Integration**: Multi-provider LLM support via [rig-core](https://github.com/0xPlaygrounds/rig) (Ollama, OpenAI, Anthropic, Gemini, OpenRouter)
- **RAG System**: Vector similarity search using embeddings for intelligent bookmark discovery
- **Content Processing**: dom_smoothie for web page content extraction
- **Browser Automation**: Browserless Chrome for reliable web page rendering and content extraction

## Model Context Protocol (MCP)

The server exposes an [MCP](https://modelcontextprotocol.io/) endpoint at `POST /mcp` so AI clients (editors, assistants, agents) can manage bookmarks and query saved content directly. It speaks the Streamable HTTP transport (protocol version `2025-11-25`) and reuses the REST API's JWT auth — every request must carry:

```
Authorization: Bearer <jwt>
```

The token is the same one returned by `POST /api/v1/login` (or `just run-cli login`). No separate MCP credentials are needed; each tool call is scoped to the authenticated user.

Available tools:

| Tool | Description |
|---|---|
| `list_bookmarks` | List all bookmarks for the user, newest first |
| `get_bookmark` | Fetch a single bookmark by id |
| `create_bookmark` | Queue a URL for ingestion; optional initial tags |
| `delete_bookmark` | Delete a bookmark by id (and its static files) |
| `list_tags` | List the user's tags with usage counts |
| `get_bookmarks_by_tag` | List bookmarks carrying a tag (case-insensitive) |
| `set_tags` | Replace a bookmark's tags |
| `append_tags` | Add tags to a bookmark, preserving existing ones |
| `search_bookmarks` | Full-text + tag-filter search |
| `list_tasks` | Paginated list of ingestion tasks (pending/done/fail) |
| `rag_query` | Ask a question answered from your bookmark content (requires an LLM provider) |
| `rag_history` | List past RAG question/answer sessions |

RAG tools (`rag_query`, `rag_history`) are only useful when an LLM provider is configured (see [LLM Provider Configuration](#llm-provider-configuration)).

### Connecting a client

Most MCP clients accept an HTTP URL + bearer header. For example, with `mcp-cli`:

```bash
mcp-cli --url http://localhost:3000/mcp --header "Authorization: Bearer $TOKEN"
```

With Claude Desktop / Cline / Cursor-style configs that take a `url`-based streamable HTTP server, set the endpoint to `http://localhost:3000/mcp` and add the `Authorization` header with the JWT obtained from the CLI login.
