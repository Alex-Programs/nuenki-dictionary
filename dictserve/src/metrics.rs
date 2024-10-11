// src/metrics.rs

use axum::{routing::get, Router};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::sync::Arc;
use std::sync::Once;
use tokio::sync::Mutex;

static INIT: Once = Once::new();
static mut RECORDER_HANDLE: Option<Arc<Mutex<PrometheusHandle>>> = None;

pub fn setup_metrics_recorder() -> Arc<Mutex<PrometheusHandle>> {
    INIT.call_once(|| {
        const LOCK_HIST: &[f64] = &[0.0001, 0.001, 0.01, 0.1, 1.0, 10.0];
        const DB_HIST: &[f64] = &[0.001, 0.01, 0.1, 1.0, 10.0];

        let recorder_handle = PrometheusBuilder::new()
            .set_buckets_for_metric(
                Matcher::Full("dict_get_item_duration_seconds".to_string()),
                DB_HIST,
            )
            .unwrap()
            .install_recorder()
            .unwrap();

        unsafe {
            RECORDER_HANDLE = Some(Arc::new(Mutex::new(recorder_handle)));
        }
    });

    unsafe { RECORDER_HANDLE.clone().unwrap() }
}

pub fn metrics_app() -> Router {
    let recorder_handle = setup_metrics_recorder();
    Router::new().route(
        "/metrics",
        get(move || async move {
            let handle = recorder_handle.lock().await;
            handle.render()
        }),
    )
}

pub type NoLabel = &'static [(&'static str, &'static str)];
