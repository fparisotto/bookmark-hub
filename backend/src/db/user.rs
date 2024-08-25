use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use postgres_from_row::FromRow;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use crate::db::{Error, Result, ResultExt};

use super::PgPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub user_id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[instrument(skip(pool))]
pub async fn get_by_id(pool: &PgPool, id: &Uuid) -> Result<Option<User>> {
    const SQL: &str = r#"SELECT * FROM "user" WHERE user_id = $1;"#;
    let client = pool.get().await?;
    let result = client.query_opt(SQL, &[id]).await?;
    let user = result.map(|row| User::try_from_row(&row)).transpose()?;
    Ok(user)
}

#[instrument(skip(pool))]
pub async fn get_by_email(pool: &PgPool, email: String) -> Result<Option<User>> {
    const SQL: &str = r#"SELECT * from "user" WHERE email = $1;"#;
    let client = pool.get().await?;
    let result = client.query_opt(SQL, &[&email]).await?;
    let user = result.map(|row| User::try_from_row(&row)).transpose()?;
    Ok(user)
}

#[instrument(skip(pool, password_hash))]
pub async fn create(pool: &PgPool, email: String, password_hash: String) -> Result<User> {
    const SQL: &str =
        r#"INSERT INTO "user" (email, password_hash) VALUES ($1, $2) RETURNING "user".*;"#;
    let client = pool.get().await?;
    let row = client
        .query_one(SQL, &[&email, &password_hash])
        .await
        .on_constraint("user_email_unique", |_| {
            Error::constraint_violation("unique_email", "email already used")
        })?;
    let user = User::try_from_row(&row)?;
    tracing::info!("User created, email={}", &user.email);
    Ok(user)
}
