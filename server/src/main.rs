use anyhow::bail;
use axum::{Extension, Router};
use axum_otel_metrics::HttpMetricsLayerBuilder;
use clap::Parser;
use server::db::PgPool;
use server::{daemon, db, endpoints, AppContext, Config};
use std::io;
use std::sync::Arc;
use tokio::signal::unix::SignalKind;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

    let pool = db::get_pool(config.pg.clone()).await?;
    db::run_migrations(&pool).await?;

    let (tx, rx) = tokio::sync::watch::channel(());
    let daemon = tokio::spawn(setup_daemon(config.clone(), pool.clone(), rx));
    let app_server = setup_app(&config, pool.clone(), tx);
    tokio::select! {
        result = app_server => {
            if let Err(error) = result {
                tracing::error!(?error, "App server error");
                std::process::exit(1);
            }
        },
        result = daemon => {
            match result {
                Ok(Err(error)) => {
                    tracing::error!(?error, "Daemon task error");
                    std::process::exit(1);
                },
                Err(error) => {
                    tracing::error!(?error, "Join error in daemon task");
                    std::process::exit(1);
                },
                Ok(Ok(_)) => {}
            }
        },
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
    tracing::debug!("signal received, starting graceful shutdown")
}

async fn setup_app(
    config: &Config,
    pool: PgPool,
    tx: tokio::sync::watch::Sender<()>,
) -> anyhow::Result<()> {
    let app_state = AppContext {
        config: Arc::new(config.clone()),
        pool,
        tx_new_task: tx,
    };
    let metrics = HttpMetricsLayerBuilder::new()
        .with_service_name("bookmark-rs".to_string())
        .build();
    let app = Router::new()
        .nest_service("/", ServeDir::new(env!("SPA_DIST")))
        .nest("/api/v1", endpoints::routers_v1())
        .merge(endpoints::health_check())
        .merge(endpoints::static_content(config))
        .merge(metrics.routes())
        .layer(metrics)
        .layer(Extension(app_state))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!("Listening on {}", &config.bind);
    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn setup_daemon(
    config: Config,
    pool: PgPool,
    rx: tokio::sync::watch::Receiver<()>,
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
        tracing::info!("Data dir is valid");
    }
    daemon::add_bookmark::run(&pool, &config, rx).await
}
