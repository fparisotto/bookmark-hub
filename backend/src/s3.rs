use anyhow::{Error, Result};
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
    let s3_config = aws_sdk_s3::config::Builder::new()
        .endpoint_url(config.s3_endpoint.as_str())
        .credentials_provider(credentials)
        .region(Region::new(config.s3_region.clone()))
        .force_path_style(true) // apply bucketname as path param instead of pre-domain
        .build();
    let client = S3Client::from_conf(s3_config);
    Ok(client)
}

pub async fn check_bucket(client: &S3Client, bucket_name: &str) -> Result<(), Error> {
    let _ = client.list_objects_v2().bucket(bucket_name).send().await?;
    tracing::info!("Bucket '{bucket_name}' found");
    Ok(())
}
