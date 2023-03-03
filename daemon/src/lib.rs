use anyhow::{Context, Result};
use std::env;
use std::str::FromStr;
use strum_macros::EnumString;
use url::Url;

pub mod processor;
pub mod runner;

#[derive(Debug, EnumString)]
pub enum Env {
    PROD,
    DEV,
}

#[derive(Debug)]
pub struct Config {
    pub app_env: Env,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub readability_endpoint: String,
    pub database_url: String,
    pub database_connection_pool_size: u32,
    pub external_s3_endpoint: String,
    pub loki_url: Option<Url>,
}

impl Config {
    fn required_env(key: &str) -> Result<String> {
        env::var(key).context(format!("missing env {key}"))
    }

    pub fn parse() -> anyhow::Result<Self> {
        let app_env: Env = Env::from_str(&Config::required_env("APP_ENV")?)?;
        let s3_access_key = Config::required_env("S3_ACCESS_KEY")?;
        let s3_secret_key = Config::required_env("S3_SECRET_KEY")?;
        let s3_endpoint = Config::required_env("S3_ENDPOINT")?;
        let s3_region = Config::required_env("S3_REGION")?;
        let s3_bucket = Config::required_env("S3_BUCKET")?;
        let readability_endpoint = Config::required_env("READABILITY_ENDPOINT")?;
        let database_url = Config::required_env("DATABASE_URL")?;
        let database_connection_pool_size: u32 =
            Config::required_env("DATABASE_CONNECTION_POOL_SIZE")?.parse()?;
        let external_s3_endpoint = Config::required_env("EXTERNAL_S3_ENDPOINT")?;
        let loki_url = std::env::var("LOKI_URL")
            .ok()
            .and_then(|url| Url::parse(&url).ok());
        Ok(Self {
            app_env,
            s3_access_key,
            s3_secret_key,
            s3_endpoint,
            s3_region,
            s3_bucket,
            readability_endpoint,
            database_url,
            database_connection_pool_size,
            external_s3_endpoint,
            loki_url,
        })
    }
}
