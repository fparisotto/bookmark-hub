use anyhow::{Context, Result};
use std::env;

pub mod processor;
pub mod runner;

#[derive(Debug)]
pub struct Config {
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub readability_endpoint: String,
    pub database_url: String,
    pub database_connection_pool_size: u32,
    pub external_s3_endpoint: String,
}

impl Config {
    fn env(key: &str) -> Result<String> {
        env::var(key).context(format!("missing env {key}"))
    }

    pub fn parse() -> anyhow::Result<Self> {
        let s3_access_key = Config::env("S3_ACCESS_KEY")?;
        let s3_secret_key = Config::env("S3_SECRET_KEY")?;
        let s3_endpoint = Config::env("S3_ENDPOINT")?;
        let s3_region = Config::env("S3_REGION")?;
        let s3_bucket = Config::env("S3_BUCKET")?;
        let readability_endpoint = Config::env("READABILITY_ENDPOINT")?;
        let database_url = Config::env("DATABASE_URL")?;
        let database_connection_pool_size: u32 =
            Config::env("DATABASE_CONNECTION_POOL_SIZE")?.parse()?;
        let external_s3_endpoint = Config::env("EXTERNAL_S3_ENDPOINT")?;
        Ok(Self {
            s3_access_key,
            s3_secret_key,
            s3_endpoint,
            s3_region,
            s3_bucket,
            readability_endpoint,
            database_url,
            database_connection_pool_size,
            external_s3_endpoint,
        })
    }
}
