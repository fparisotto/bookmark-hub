# Bookmark Hub

A self-hosted bookmark management application that helps you organize, search, and discover your saved links. Built with Rust, featuring AI-powered summarization and tagging to make your bookmarks more useful and discoverable.

## Features

- **Offline-First**: Store and manage bookmarks entirely on your own infrastructure
- **AI-Powered Organization**: Automatic tagging and summarization using Ollama integration
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
- Configures embedding model as `nomic-embed-text:v1.5` for RAG features

### Development Setup

For local development with hot reloading:

#### Prerequisites
- Rust toolchain with `wasm32-unknown-unknown` target
- [Just](https://github.com/casey/just) task runner
- [Trunk](https://trunkrs.dev/) for WebAssembly builds
- PostgreSQL database
- Optional: [Ollama](https://ollama.ai/) for AI features

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

# Optional: AI Features
export OLLAMA_URL=http://localhost:11434
export OLLAMA_TEXT_MODEL=gemma3:4b
export OLLAMA_EMBEDDING_MODEL=nomic-embed-text:v1.5
```

### CLI Usage

```bash
# Import bookmarks
$ just run-cli import --file bookmarks.html

# Export bookmarks
$ just run-cli export --format json > bookmarks.json

# Add a bookmark
$ just run-cli add --url https://example.com --tags "rust,webdev"
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
- **AI Integration**: Ollama for content summarization, tag generation, and embedding creation
- **RAG System**: Vector similarity search using embeddings for intelligent bookmark discovery
- **Content Processing**: dom_smoothie for web page content extraction
