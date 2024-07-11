use axum::{
    extract::{MatchedPath, Request},
    middleware::{self, Next},
    response::IntoResponse,
    routing::get,
    Extension, Router,
};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder};
use std::{future::ready, time::Instant};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use sqlx::{Pool, Postgres};

use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use tokio::signal::unix::SignalKind;

use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use tracing_subscriber::EnvFilter;

use public_api::{database, endpoints, AppContext, Config, Env};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse()?;
    setup_tracing(&config)?;

    let db: Pool<Postgres> = database::connect(&config).await?;
    database::run_migrations(&db).await?;

    let app_server = setup_app(config, db);
    let metrics_server = setup_metrics_server();
    tokio::select! {
        _ = app_server => {}
        _ = metrics_server => {}
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

async fn setup_app(config: Config, db: Pool<Postgres>) -> anyhow::Result<()> {
    let app_state = AppContext {
        config: Arc::new(config),
        db,
    };

    let cors_layer = CorsLayer::new()
        .allow_origin([
            "https://bookmark.k8s.home".parse()?,
            "http://localhost:8080".parse()?,
        ])
        .allow_methods(Any)
        .allow_headers(Any);

    let routes = endpoints::routers_v1().route_layer(middleware::from_fn(track_metrics));

    let app = Router::new()
        .nest("/api/v1", routes)
        .layer(Extension(app_state))
        .layer(cors_layer)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("Server running, listening on {:?}", &listener);
    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
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
            // TODO expose in k8s deployment the desired information
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

async fn setup_metrics_server() -> anyhow::Result<()> {
    const EXPONENTIAL_SECONDS: &[f64] = &[
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ];

    let handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("http_requests_duration_seconds".to_string()),
            EXPONENTIAL_SECONDS,
        )?
        .install_recorder()?;

    let router = Router::new().route("/metrics", get(move || ready(handle.render())));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9090").await?;
    tracing::debug!("Metrics server listening on {:?}", &listener);
    axum::serve(listener, router.into_make_service()).await?;

    Ok(())
}

async fn track_metrics(req: Request, next: Next) -> impl IntoResponse {
    let start = Instant::now();
    let path = if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
        matched_path.as_str().to_owned()
    } else {
        req.uri().path().to_owned()
    };
    let method = req.method().clone();

    let response = next.run(req).await;

    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let labels = [
        ("method", method.to_string()),
        ("path", path),
        ("status", status),
    ];

    metrics::counter!("http_requests_total", &labels).increment(1);
    metrics::histogram!("http_requests_duration_seconds", &labels).record(latency);

    response
}
