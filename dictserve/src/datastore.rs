use crate::metrics::NoLabel;
use dashmap::DashMap;
use metrics::histogram;
use sqlx::PgPool;
use std::time::Instant;
use tokio::sync::RwLock;

use tracing::{debug, info};

pub struct SyncedLocalDataStore {
    tokens_to_user_ids: DashMap<String, i32>,
    mode_lock: RwLock<()>,
    pool: PgPool,
}

impl SyncedLocalDataStore {
    pub async fn new(pool: PgPool) -> Self {
        // Pull data from the database and populate the local cache
        let tokens_to_user_ids = DashMap::new();

        let sessions = sqlx::query!("SELECT token, user_id FROM sessions")
            .fetch_all(&pool)
            .await
            .expect("Failed to fetch sessions from database");

        for session in sessions {
            info!("Populating session: {:?}", session);

            tokens_to_user_ids.insert(session.token, session.user_id);
        }

        Self {
            tokens_to_user_ids,
            mode_lock: RwLock::new(()),
            pool,
        }
    }

    pub async fn insert_new_session(&self, token: String, user_id: i32) {
        // yes we're writing but this is an atomic write that we just don't
        // want to do when we're doing db ops
        let _read_lock = self.mode_lock.read().await;

        self.tokens_to_user_ids.insert(token, user_id);
    }

    pub async fn get_user_id(&self, token: &str) -> Option<i32> {
        let start_t = Instant::now();
        let _read_lock = self.mode_lock.read().await;
        histogram!("dict_get_read_lock_duration_seconds", &[] as NoLabel)
            .record(start_t.elapsed().as_secs_f64());

        self.tokens_to_user_ids.get(token).map(|x| *x)
    }

    pub async fn reconcile_with_db(&self) -> Result<(), sqlx::Error> {
        let start_update_time = std::time::Instant::now();

        let start_acquire_time = std::time::Instant::now();
        let _write_lock = self.mode_lock.write().await;
        histogram!("dict_get_write_lock_duration_seconds", &[] as NoLabel)
            .record(start_acquire_time.elapsed().as_secs_f64());

        // Refresh the local cache
        let sessions = sqlx::query!("SELECT token, user_id FROM sessions")
            .fetch_all(&self.pool)
            .await?;

        // Clear current local store
        self.tokens_to_user_ids.clear();

        for session in sessions {
            debug!("Reconciling-back session: {:?}", session);

            self.tokens_to_user_ids
                .insert(session.token, session.user_id);
        }

        histogram!("dict_reconcile_db_duration_seconds", &[] as NoLabel)
            .record(start_update_time.elapsed().as_secs_f64());

        Ok(())
    }
}
