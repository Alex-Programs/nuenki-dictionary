mod config;
mod database;
mod datastore;
mod dictionary;
mod get_definition;
mod metrics;
mod utils;

#[macro_use]
extern crate savefile_derive;

use config::Config;
use dictionary::DictionaryStore;
use sqlx::PgPool;

use axum::routing::{get, post};
use axum::Router;

use tokio::net::TcpListener;

use tracing::{debug, error, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

use tracing::level_filters::LevelFilter;
use tracing_loki::url::Url;
use tracing_subscriber::prelude::*;

use std::sync::Arc;

use datastore::SyncedLocalDataStore;

#[derive(Clone)]
pub struct AppState {
    pg_pool: PgPool,
    store: Arc<SyncedLocalDataStore>,
    config: Config,
    dictionary_store: Arc<DictionaryStore>,
}

#[tokio::main]
async fn main() {
    let config = Config::from_file("./Config/config.toml").unwrap();
    let cloned_conf = config.clone();

    let mut labels_map = std::collections::HashMap::new();
    labels_map.insert("job".to_string(), config.loki_job.clone());

    let empty_map = std::collections::HashMap::new();

    let (loki_layer, task) =
        tracing_loki::layer(Url::parse(&config.loki_url).unwrap(), labels_map, empty_map).unwrap();

    let loki_layer = loki_layer.with_filter(LevelFilter::INFO);

    let console_layer = tracing_subscriber::fmt::layer()
        .pretty()
        .with_filter(LevelFilter::INFO);

    tracing_subscriber::registry()
        .with(loki_layer)
        .with(console_layer)
        .init();

    let _ = tokio::spawn(task);

    debug!("Config loaded");

    let pg_pool = database::create_pool(&config.database_url, config.max_db_connections).await;

    debug!("PG pool created");

    let store = Arc::new(SyncedLocalDataStore::new(pg_pool.clone()).await);

    let store_clone = store.clone();
    let reconcile_interval = config.reconcile_interval;

    tokio::spawn(async move {
        intermittent_reconcile(store_clone, reconcile_interval).await;
    });

    debug!("Reconciliation task started");

    info!("Creating in-memory dictionary...");
    let dict_store = dictionary::DictionaryStore::from_elements_dump(config.dump_path);

    let app = Router::new()
        .route("/get_definition", get(get_definition::get_definition))
        .with_state(AppState {
            pg_pool,
            store,
            config: cloned_conf,
            dictionary_store: Arc::new(dict_store),
        });

    debug!("App initialised");

    let metrics_listener = TcpListener::bind(config.metrics_bind.clone())
        .await
        .expect("Failed to bind metrics server");

    tokio::spawn(async move {
        axum::serve(metrics_listener, metrics::metrics_app())
            .await
            .unwrap();
    });

    info!("Metrics server started on {}", config.metrics_bind);

    let listener = TcpListener::bind(format!("{}:{}", config.listen_address, config.listen_port))
        .await
        .expect("Failed to bind");

    info!(
        "Serving on {}:{}",
        config.listen_address, config.listen_port
    );

    axum::serve(listener, app)
        .await
        .expect("Failed to start serving");
}

async fn intermittent_reconcile(datastore: Arc<SyncedLocalDataStore>, interval: f32) {
    loop {
        let start = std::time::Instant::now();
        let result = datastore.reconcile_with_db().await;
        match result {
            Ok(_) => {}
            Err(e) => {
                error!("Error reconciling with DB: {:?}", e);

                // wait half the time before trying again
                tokio::time::sleep(tokio::time::Duration::from_secs_f32(interval / 2.0)).await;

                continue;
            }
        }

        let elapsed = start.elapsed().as_secs_f32();

        if elapsed < interval {
            tokio::time::sleep(tokio::time::Duration::from_secs_f32(interval - elapsed)).await;
        }
    }
}
