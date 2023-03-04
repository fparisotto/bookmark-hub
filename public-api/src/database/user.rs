use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres};
use tracing::instrument;
use uuid::Uuid;

use crate::database::{Error, Result, ResultExt};

#[derive(sqlx::FromRow)]
pub struct User {
    pub user_id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct UserTable;

impl UserTable {
    #[instrument(skip(db))]
    pub async fn get_by_id(db: &Pool<Postgres>, id: &Uuid) -> Result<Option<User>> {
        let result: Option<User> = sqlx::query_as(r#"select * from "user" where user_id = $1"#)
            .bind(id)
            .fetch_optional(db)
            .await?;
        Ok(result)
    }

    #[instrument(skip(db))]
    pub async fn get_by_email(db: &Pool<Postgres>, email: &String) -> Result<Option<User>> {
        let result: Option<User> = sqlx::query_as(r#"select * from "user" where email = $1"#)
            .bind(email)
            .fetch_optional(db)
            .await?;
        Ok(result)
    }

    #[instrument(skip(db, password_hash))]
    pub async fn create(db: &Pool<Postgres>, email: String, password_hash: String) -> Result<User> {
        let user: User = sqlx::query_as(
            r#"insert into "user" (email, password_hash) values ($1, $2) returning "user".*;"#,
        )
        .bind(email)
        .bind(password_hash)
        .fetch_one(db)
        .await
        .on_constraint("user_email_unique", |_| {
            Error::constraint_violation("unique_email", "email already used")
        })?;
        tracing::info!("User created, email={}", &user.email);
        Ok(user)
    }
}
