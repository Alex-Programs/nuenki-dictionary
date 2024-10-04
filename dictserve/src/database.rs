use crate::utils::GenericFetchError;
use chrono::NaiveDateTime;
use rand::Rng;
use sqlx::postgres::PgPoolOptions;
use sqlx::FromRow;
use sqlx::PgPool;

pub async fn create_pool(database_url: &str, max_connections: u32) -> PgPool {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await
        .expect("DB connection failure")
}

#[derive(FromRow)]
pub struct User {
    pub id: i32,
}

#[derive(FromRow)]
pub struct UserSession {
    pub id: i32,
    pub user_id: i32,
    pub token: String,
    pub created_at: NaiveDateTime,
}

pub async fn get_user_by_token(pool: &PgPool, token: &str) -> Result<User, GenericFetchError> {
    let result = sqlx::query_as!(
        User,
        r#"
        SELECT u.id
        FROM users u
        JOIN sessions s ON s.user_id = u.id
        WHERE s.token = $1
        "#,
        token
    )
    .fetch_one(pool)
    .await;

    match result {
        Ok(user) => Ok(user),
        Err(sqlx::Error::RowNotFound) => Err(GenericFetchError::NotFound),
        Err(e) => Err(GenericFetchError::PostgresError(e)),
    }
}
