use axum::{Extension, Router};
use axum_otel_metrics::HttpMetricsLayerBuilder;
use clap::Parser;
use public_api::{daemon, database, endpoints, s3, AppContext, Config, Env};
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

    let db: Pool<Postgres> = database::connect(&config).await?;
    database::run_migrations(&db).await?;

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
    let s3_client = s3::make_s3_client(&config).await?;
    let http: HttpClient = HttpClient::new();
    s3::check_bucket(&s3_client, &config.s3_bucket).await?;
    daemon::run(&db, &http, &s3_client, &config).await
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
