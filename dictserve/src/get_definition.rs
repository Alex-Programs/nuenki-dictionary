use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};

use crate::metrics::NoLabel;
use metrics::{counter, histogram};
use std::time::Instant;
use Languages::TargetLanguage;

use libdictdefinition::DictionaryElementData;

#[derive(Serialize, Deserialize, Debug)]
pub struct DictionaryResponse {
    element: DictionaryElementData,
    wiktionary_link: String,
}

#[derive(Deserialize)]
pub struct DictionaryRequest {
    language: TargetLanguage,
    word: String,
}

pub async fn get_definition(
    State(state): State<AppState>,
    Query(payload): Query<DictionaryRequest>,
) -> Result<Json<DictionaryResponse>, (StatusCode, String)> {
    let label = [("language", payload.language.to_extension_technical_format())];
    counter!("dictionary_query_language", &label).increment(1);

    let t_start = Instant::now();
    let dict_element = state
        .dictionary_store
        .query(payload.language.clone(), &payload.word);
    let t_taken = t_start.elapsed();

    histogram!("dict_get_item_duration_seconds", &[] as NoLabel).record(t_taken.as_secs_f64());

    match dict_element {
        Some(element) => {
            let label = [("status", "success")];
            counter!("dictionary_query_status", &label).increment(1);

            Ok(Json(DictionaryResponse {
                wiktionary_link: element.get_wiktionary_link(),
                element,
            }))
        }
        None => {
            let label = [("language", "not_found")];
            counter!("dictionary_query_status", &label).increment(1);

            Err((StatusCode::NOT_FOUND, "Word not found".to_string()))
        }
    }
}
