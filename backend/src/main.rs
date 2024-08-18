use anyhow::bail;
use axum::{Extension, Router};
use axum_otel_metrics::HttpMetricsLayerBuilder;
use backend::{daemon, db, endpoints, AppContext, Config, Env};
use clap::Parser;
use reqwest::Client as HttpClient;
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use tokio::signal::unix::SignalKind;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();
    setup_tracing(&config)?;

    let db: Pool<Postgres> = db::connect(&config).await?;
    db::run_migrations(&db).await?;

    let app_server = setup_app(&config, db.clone());
    let daemon = tokio::spawn(setup_daemon(config.clone(), db));
    tokio::select! {
        result = app_server => {
            if let Err(error) = result {
                tracing::error!(?error, "App server error");
            }
        },
        result = daemon => {
            match result {
                Ok(Err(error)) => {
                    tracing::error!(?error, "Daemon task error");
                },
                Err(error) => {
                    tracing::error!(?error, "Join error in daemon task");
                },
                _ => {}
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

async fn setup_app(config: &Config, db: Pool<Postgres>) -> anyhow::Result<()> {
    let app_state = AppContext {
        config: Arc::new(config.clone()),
        db,
    };
    let metrics = HttpMetricsLayerBuilder::new()
        .with_service_name("bookmark-rs".to_string())
        .build();
    let app = Router::new()
        .nest("/api/v1", endpoints::routers_v1())
        .merge(endpoints::health_check())
        .merge(endpoints::static_content(config))
        .merge(metrics.routes())
        .layer(metrics)
        .layer(Extension(app_state))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!("Listening on {:?}", &listener);
    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn setup_daemon(config: Config, db: Pool<Postgres>) -> anyhow::Result<()> {
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
    let http: HttpClient = HttpClient::new();
    daemon::run(&db, &http, &config).await
}

fn setup_tracing(config: &Config) -> anyhow::Result<()> {
    let tracing_setup = tracing_subscriber::registry().with(EnvFilter::from_default_env());
    match config.app_env {
        Env::DEV => {
            tracing_setup.with(tracing_subscriber::fmt::layer()).init();
        }
        Env::PROD => {
            let loki_url = config
                .loki_url
                .clone()
                .expect("'LOKI_URL' env var need to be set in APP_ENV=PROD");
            // Prefix with 'LOKI_LABEL_' to expose desired information in k8s deployment
            // https://kubernetes.io/docs/tasks/inject-data-application/environment-variable-expose-pod-information/
            let labels: HashMap<String, String> = std::env::vars()
                .filter(|(key, _)| key.starts_with("LOKI_LABEL_"))
                .map(|(key, value)| (key.replace("LOKI_LABEL_", "").to_lowercase(), value))
                .collect();
            let (layer, task) = tracing_loki::layer(loki_url, labels, HashMap::new())?;
            tracing_setup.with(layer).init();
            tokio::spawn(task);
        }
    }
    Ok(())
}
