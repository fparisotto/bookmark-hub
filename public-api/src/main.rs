use axum::extract::MatchedPath;
use axum::http::Request;
use axum::middleware;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Extension;
use axum::Router;

use metrics_exporter_prometheus::Matcher;
use metrics_exporter_prometheus::PrometheusBuilder;

use sqlx::{Pool, Postgres};

use std::collections::HashMap;
use std::future::ready;
use std::io;
use std::time::Instant;
use std::{net::SocketAddr, sync::Arc};

use tokio::signal::unix::SignalKind;

use tower_http::cors::{Any, CorsLayer, Origin};
use tower_http::trace::TraceLayer;

use tracing_subscriber::{prelude::*, EnvFilter};

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
        .allow_origin(Origin::list(vec![
            "https://bookmark.k8s.home".parse().unwrap(),
            "http://localhost:*".parse().unwrap(),
        ]))
        .allow_methods(Any)
        .allow_headers(Any);

    let routes = endpoints::routers_v1().route_layer(middleware::from_fn(track_metrics));

    let app = Router::new()
        .nest("/api/v1", routes)
        .layer(Extension(app_state))
        .layer(cors_layer)
        .layer(TraceLayer::new_for_http());

    let address = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server running, listening on {}", address);
    axum::Server::bind(&address)
        .serve(app.into_make_service())
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
            let (layer, task) = tracing_loki::layer(loki_url, HashMap::new(), HashMap::new())?;
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
        )
        .unwrap()
        .install_recorder()?;

    let router = Router::new().route("/metrics", get(move || ready(handle.render())));

    let addr = SocketAddr::from(([127, 0, 0, 1], 9090));
    tracing::debug!("Metrics server listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await?;

    Ok(())
}

async fn track_metrics<B>(req: Request<B>, next: Next<B>) -> impl IntoResponse {
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

    metrics::increment_counter!("http_requests_total", &labels);
    metrics::histogram!("http_requests_duration_seconds", latency, &labels);

    response
}
