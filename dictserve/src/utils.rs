use crate::database::get_user_by_token;
use crate::datastore::SyncedLocalDataStore;
use sqlx::PgPool;
use tracing::info;

#[derive(Debug)]
pub enum GenericFetchError {
    PostgresError(sqlx::Error),
    NotFound,
}

pub async fn get_userid_by_token_quickly(
    pg_conn: &mut PgPool,
    datastore: &SyncedLocalDataStore,
    token: &str,
) -> Result<i32, GenericFetchError> {
    let by_id = datastore.get_user_id(token).await;

    if let Some(id) = by_id {
        return Ok(id);
    }

    info!(
        "User with token {} not found in local cache, fetching from postgres",
        token
    );

    let user = get_user_by_token(pg_conn, token).await?;

    Ok(user.id)
}
