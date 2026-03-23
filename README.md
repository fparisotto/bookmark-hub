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
export LLM_EMBEDDING_DIMENSION=1024
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
export LLM_EMBEDDING_DIMENSION=768
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

| Variable | Default | Description |
|---|---|---|
| `LLM_PROVIDER` | `ollama` | Text completion provider |
| `LLM_TEXT_MODEL` | _(none, disables AI)_ | Model for text tasks |
| `LLM_EMBEDDING_PROVIDER` | _(falls back to LLM_PROVIDER)_ | Embedding provider |
| `LLM_EMBEDDING_MODEL` | _(falls back to LLM_TEXT_MODEL)_ | Model for embeddings |
| `LLM_EMBEDDING_DIMENSION` | `1024` | Embedding vector dimension |
| `LLM_EMBEDDING_API_KEY` | _(none)_ | API key for embedding provider if different |
| `LLM_REQUEST_TIMEOUT_SECS` | `120` | HTTP request timeout for LLM calls |
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
