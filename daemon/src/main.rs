use std::collections::HashMap;

use anyhow::{Error, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{Client as S3Client, Credentials, Endpoint, Region};
use daemon::{runner, Config, Env};
use reqwest::Client as HttpClient;
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::{prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse()?;
    setup_tracing(&config)?;
    let http: HttpClient = HttpClient::new();
    let s3_client: S3Client = make_s3_client(&config).await?;
    check_bucket(&s3_client, &config.s3_bucket).await?;
    let connection_pool: sqlx::Pool<sqlx::Postgres> = PgPoolOptions::new()
        .max_connections(config.database_connection_pool_size)
        .connect(&config.database_url)
        .await?;
    runner::run(&connection_pool, &http, &s3_client, &config).await?;
    Ok(())
}

async fn make_s3_client(config: &Config) -> Result<S3Client, Error> {
    let credentials = Credentials::new(
        config.s3_access_key.clone(),
        config.s3_secret_key.clone(),
        None,
        None,
        "daemon",
    );
    let region = RegionProviderChain::first_try(Region::new(config.s3_region.clone()));
    let endpoint = Endpoint::immutable(&config.s3_endpoint)?;
    let config = aws_config::from_env()
        .region(region)
        .endpoint_resolver(endpoint)
        .credentials_provider(credentials)
        .load()
        .await;
    let client = S3Client::new(&config);
    Ok(client)
}

async fn check_bucket(client: &S3Client, bucket_name: &str) -> Result<(), Error> {
    let _ = client.list_objects_v2().bucket(bucket_name).send().await?;
    tracing::info!("Bucket found");
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
