use dashmap::DashMap;
use rayon::prelude::*;
use savefile::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::info;
use Languages::TargetLanguage;

#[derive(Clone, Debug, Savefile, Serialize, Deserialize)]
pub struct Definition {
    text: String,
    tags: Vec<String>,
}

#[derive(Clone, Debug, Savefile, Deserialize, Serialize)]
pub struct DictionaryElement {
    word: String,
    lang: TargetLanguage,
    audio: Vec<String>,      // Can hold multiple audio links
    ipa: Option<String>,     // Optional, as there might be none
    word_types: Vec<String>, // Can hold multiple word types
    definitions: Vec<Definition>,
}

impl DictionaryElement {
    pub fn get_wiktionary_link(&self) -> String {
        let encoded_word = self.word.replace(" ", "");

        format!(
            "https://en.wiktionary.org/wiki/{}#{}",
            encoded_word,
            self.lang.to_wiktionary_language_code()
        )
    }
}

pub struct DictionaryStore {
    datastore: DashMap<(TargetLanguage, String), DictionaryElement>,
}

impl DictionaryStore {
    pub fn from_elements_dump(path: String) -> Self {
        let start_t = Instant::now();

        let elements: Result<Vec<DictionaryElement>, SavefileError> = load_file(path, 0);
        let elements = elements.unwrap();
        let e_c = elements.len();

        let store: DashMap<(TargetLanguage, String), DictionaryElement> = DashMap::new();

        // Use rayon to parallelize insertion
        elements.into_par_iter().for_each(|element| {
            store.insert((element.lang.clone(), element.word.clone()), element);
        });

        let time_taken = start_t.elapsed();

        info!(
            "Loaded {} items from elements dump in {}s",
            e_c,
            time_taken.as_secs_f32()
        );

        Self { datastore: store }
    }

    pub fn query(&self, lang: TargetLanguage, word: &str) -> Option<DictionaryElement> {
        let key = (lang.clone(), word.to_string());
        println!("Querying with key: {:?}", key);

        // Try querying with the original word
        if let Some(value) = self.datastore.get(&key) {
            println!("Found value for original word");
            return Some(value.clone());
        }

        // If not found and the word isn't all lowercase, try again with the lowercase word
        if word != word.to_lowercase() {
            let lower_key = (lang, word.to_lowercase());
            println!("Trying lowercase key: {:?}", lower_key);
            if let Some(value) = self.datastore.get(&lower_key) {
                println!("Found value for lowercase word");
                return Some(value.clone());
            }
        }

        println!("No value found for word: {}", word);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_query_word() {
        let path = "../build_dump/dict.bin".to_string();
        let store = DictionaryStore::from_elements_dump(path);

        // Test querying a word that should exist
        let lang = TargetLanguage::French;
        let word = "flambes";
        let result = store.query(lang.clone(), word);
        println!("Query result for '{}': {:?}", word, result);
        assert!(result.is_some());

        // Test querying a word that doesn't exist
        let missing_word = "nonexistent_word";
        let result = store.query(lang, missing_word);
        println!("Query result for '{}': {:?}", missing_word, result);
        assert!(result.is_none());
    }
}
