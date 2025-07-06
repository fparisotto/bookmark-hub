use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use postgres_from_row::FromRow;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{Error, Result, ResultExt};

use super::PgPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub user_id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn get_by_id(pool: &PgPool, id: &Uuid) -> Result<Option<User>> {
    const SQL: &str = r#"SELECT * FROM "user" WHERE user_id = $1;"#;
    let client = pool.get().await?;
    let result = client.query_opt(SQL, &[id]).await?;
    let user = result.map(|row| User::try_from_row(&row)).transpose()?;
    Ok(user)
}

pub async fn get_by_username(pool: &PgPool, username: String) -> Result<Option<User>> {
    const SQL: &str = r#"SELECT * from "user" WHERE username = $1;"#;
    let client = pool.get().await?;
    let result = client.query_opt(SQL, &[&username]).await?;
    let user = result.map(|row| User::try_from_row(&row)).transpose()?;
    Ok(user)
}

pub async fn create(pool: &PgPool, username: String, password_hash: String) -> Result<User> {
    const SQL: &str =
        r#"INSERT INTO "user" (username, password_hash) VALUES ($1, $2) RETURNING "user".*;"#;
    let client = pool.get().await?;
    let row = client
        .query_one(SQL, &[&username, &password_hash])
        .await
        .on_constraint("user_username_unique", |_| {
            Error::constraint_violation("unique_username", "username already used")
        })?;
    let user = User::try_from_row(&row)?;
    tracing::info!("User created, username={}", &user.username);
    Ok(user)
}
