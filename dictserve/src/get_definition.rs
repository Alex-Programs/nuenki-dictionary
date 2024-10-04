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

use crate::dictionary::DictionaryElement;

#[derive(Serialize, Deserialize, Debug)]
pub struct DictionaryResponse {
    element: DictionaryElement,
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
    let label = [("language", payload.language.to_nice_format())];
    counter!("dictionary_query_language", &label).increment(1);

    let dict_element = state
        .dictionary_store
        .query(payload.language.clone(), &payload.word);

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
