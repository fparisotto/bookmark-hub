use anyhow::{Error, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_config::BehaviorVersion;
use aws_sdk_s3::config::Credentials;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::Client as S3Client;
use secrecy::ExposeSecret;

use crate::Config;

pub async fn s3_client(config: &Config) -> Result<S3Client, Error> {
    let credentials = Credentials::new(
        config.s3_access_key.expose_secret(),
        config.s3_secret_key.expose_secret(),
        None,
        None,
        "backend",
    );
    let region = RegionProviderChain::first_try(Region::new(config.s3_region.clone()));
    let config = aws_config::defaults(BehaviorVersion::v2024_03_28())
        .region(region)
        .endpoint_url(config.s3_endpoint.as_str())
        .credentials_provider(credentials)
        .load()
        .await;
    let client = S3Client::new(&config);
    Ok(client)
}

pub async fn check_bucket(client: &S3Client, bucket_name: &str) -> Result<(), Error> {
    let _ = client.list_objects_v2().bucket(bucket_name).send().await?;
    tracing::info!("Bucket '{bucket_name}' found");
    Ok(())
}
