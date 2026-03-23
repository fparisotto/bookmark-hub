use std::future::pending;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use anyhow::bail;
use axum::http::{header, HeaderValue, Method};
use axum::{Extension, Router};
use axum_otel_metrics::HttpMetricsLayerBuilder;
use clap::Parser;
use server::db::PgPool;
use server::llm::LlmClient;
use server::{daemon, db, endpoints, AppContext, Config};
use tokio::signal::unix::SignalKind;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::level_filters::LevelFilter;
use tracing::{debug, error, info, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    info!(?config, "Starting Bookmark Hub Server");

    let llm_client = server::llm::build_llm_client(&config.llm)?;
    let llm_enabled = llm_client.is_some();
    info!(llm_enabled = %llm_enabled, provider = %config.llm.llm_provider, "LLM AI features configuration");
    if let Some(ref client) = llm_client {
        debug!(
            text_model = %client.text_model,
            embedding_model = %client.embedding_model,
            embedding_ndims = %client.embedding_ndims,
            "LLM client configured"
        );
    }

    info!("Initializing database connection pool");
    let pool = db::get_pool(config.pg.clone()).await?;
    info!("Running database migrations");
    db::run_migrations(&pool).await?;

    // Ensure embedding dimension matches configuration
    if let Some(ref client) = llm_client {
        db::ensure_embedding_dimension(&pool, client.embedding_ndims).await?;
    }

    info!("Database initialization complete");

    debug!("Creating inter-daemon communication channels");
    let (new_task_tx, new_task_rx) = tokio::sync::watch::channel(());
    let (new_bookmark_tx, new_bookmark_rx) = tokio::sync::watch::channel(());

    info!("Spawning background daemons");
    let add_bookmark_daemon = tokio::spawn(setup_add_bookmark_daemon(
        config.clone(),
        pool.clone(),
        new_task_rx,
        new_bookmark_tx,
    ));
    let tags_daemon = tokio::spawn(setup_tags_daemon(
        llm_client.clone(),
        pool.clone(),
        new_bookmark_rx.clone(),
    ));
    let summary_daemon = tokio::spawn(setup_summary_daemon(
        llm_client.clone(),
        pool.clone(),
        new_bookmark_rx.clone(),
    ));
    let embeddings_daemon = tokio::spawn(setup_embeddings_daemon(
        llm_client.clone(),
        pool.clone(),
        new_bookmark_rx.clone(),
    ));

    info!("Setting up HTTP server");
    let app_server = setup_app(&config, pool.clone(), new_task_tx, llm_client);

    info!("All services started successfully");
    tokio::select! {
        result = app_server => {
            if let Err(error) = result {
                error!(?error, "App server error");
                std::process::exit(1);
            }
            info!("App server stopped");
        },
        result = add_bookmark_daemon => {
            match result {
                Ok(Err(error)) => {
                    error!(?error, "Add bookmark daemon error");
                    std::process::exit(1);
                },
                Err(error) => {
                    error!(?error, "Join error in add bookmark daemon");
                    std::process::exit(1);
                },
                Ok(Ok(_)) => {
                    info!("Add bookmark daemon stopped");
                }
            }
        },
        result = tags_daemon => {
            match result {
                Ok(Err(error)) => {
                    error!(?error, "Tags daemon error");
                    std::process::exit(1);
                },
                Err(error) => {
                    error!(?error, "Join error in tags daemon");
                    std::process::exit(1);
                },
                Ok(Ok(_)) => {
                    info!("Tags daemon stopped");
                }
            }
        }
        result = summary_daemon => {
            match result {
                Ok(Err(error)) => {
                    error!(?error, "Summary daemon error");
                    std::process::exit(1);
                },
                Err(error) => {
                    error!(?error, "Join error in summary daemon");
                    std::process::exit(1);
                },
                Ok(Ok(_)) => {
                    info!("Summary daemon stopped");
                }
            }
        }
        result = embeddings_daemon => {
            match result {
                Ok(Err(error)) => {
                    error!(?error, "Embeddings daemon error");
                    std::process::exit(1);
                },
                Err(error) => {
                    error!(?error, "Join error in embeddings daemon");
                    std::process::exit(1);
                },
                Ok(Ok(_)) => {
                    info!("Embeddings daemon stopped");
                }
            }
        }
    }
    Ok(())
}

async fn shutdown_signal() {
    async fn terminate() -> io::Result<()> {
        tokio::signal::unix::signal(SignalKind::terminate())?
            .recv()
            .await;
        Ok(())
    }
    tokio::select! {
        _ = terminate() => {},
        _ = tokio::signal::ctrl_c() => {},
    }
    info!("Shutdown signal received, starting graceful shutdown")
}

async fn setup_app(
    config: &Config,
    pool: PgPool,
    tx: tokio::sync::watch::Sender<()>,
    llm_client: Option<LlmClient>,
) -> anyhow::Result<()> {
    let app_state = AppContext {
        config: Arc::new(config.clone()),
        pool,
        auth_rate_limiter: Arc::new(server::auth_rate_limit::AuthRateLimiter::new(
            5,
            Duration::from_secs(5 * 60),
        )),
        tx_new_task: tx,
        llm_client,
    };

    let metrics = HttpMetricsLayerBuilder::new().build();
    let mut app = Router::new()
        .nest("/api/v1", endpoints::routers_v1())
        .merge(endpoints::health_check())
        .merge(endpoints::static_content(config))
        .fallback_service(ServeDir::new(env!("SPA_DIST")))
        .layer(metrics)
        .layer(Extension(app_state))
        .layer(TraceLayer::new_for_http());

    if let Some(origin) = &config.cors_allow_origin {
        let cors = if origin == "*" {
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        } else {
            CorsLayer::new()
                .allow_origin(
                    HeaderValue::from_str(origin)
                        .map_err(|_| anyhow::anyhow!("Invalid APP_CORS_ALLOW_ORIGIN value"))?,
                )
                .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        };
        app = app.layer(cors);
    }

    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    info!(bind_address = %config.bind, "HTTP server listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    info!("HTTP server shutdown complete");
    Ok(())
}

async fn setup_add_bookmark_daemon(
    config: Config,
    pool: PgPool,
    new_task_rx: tokio::sync::watch::Receiver<()>,
    new_bookmark_tx: tokio::sync::watch::Sender<()>,
) -> anyhow::Result<()> {
    let data_dir = config.data_dir.clone();
    if !data_dir.exists() || !data_dir.is_dir() {
        bail!("Data dir is not a directory, {:?}", &config.data_dir);
    }
    if data_dir.metadata()?.permissions().readonly() {
        bail!(
            "Data dir is readonly, needs write access, {:?}",
            &config.data_dir
        );
    }
    {
        let mut test_file = data_dir.clone();
        test_file.push("test.txt");
        std::fs::write(&test_file, "test data")?;
        std::fs::remove_file(&test_file)?;
        debug!(data_dir = ?data_dir, "Data directory validation successful");
    }
    info!(data_dir = ?config.data_dir, "Starting add bookmark daemon");
    daemon::add_bookmark::run(&pool, &config, new_task_rx, new_bookmark_tx).await
}

async fn setup_tags_daemon(
    llm_client: Option<LlmClient>,
    pool: PgPool,
    new_bookmark_rx: tokio::sync::watch::Receiver<()>,
) -> anyhow::Result<()> {
    match llm_client {
        Some(client) => {
            info!(model = %client.text_model, "Starting tags daemon");
            daemon::tag::run(&pool, new_bookmark_rx, &client).await
        }
        None => {
            warn!("No LLM configured, disabling tags daemon");
            pending::<anyhow::Result<()>>().await
        }
    }
}

async fn setup_summary_daemon(
    llm_client: Option<LlmClient>,
    pool: PgPool,
    new_bookmark_rx: tokio::sync::watch::Receiver<()>,
) -> anyhow::Result<()> {
    match llm_client {
        Some(client) => {
            info!(model = %client.text_model, "Starting summary daemon");
            daemon::summary::run(&pool, new_bookmark_rx, &client).await
        }
        None => {
            warn!("No LLM configured, disabling summary daemon");
            pending::<anyhow::Result<()>>().await
        }
    }
}

async fn setup_embeddings_daemon(
    llm_client: Option<LlmClient>,
    pool: PgPool,
    new_bookmark_rx: tokio::sync::watch::Receiver<()>,
) -> anyhow::Result<()> {
    match llm_client {
        Some(client) => {
            info!(
                embedding_model = %client.embedding_model,
                "Starting embeddings daemon"
            );
            daemon::embeddings::run(&pool, new_bookmark_rx, &client).await
        }
        None => {
            warn!("No LLM configured, disabling embeddings daemon");
            pending::<anyhow::Result<()>>().await
        }
    }
}
