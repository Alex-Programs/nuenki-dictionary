use dashmap::DashMap;
use libdictdefinition::{
    CompressedDictionaryElementWrapper, Definition, DictionaryElementData, HyperlinkedText,
};
use rayon::prelude::*;
use std::fs::File;
use std::io::Read;
use std::time::Instant;
use tracing::info;
use zstd::stream::decode_all;
use Languages::TargetLanguage;

pub struct DictionaryStore {
    datastore: DashMap<(TargetLanguage, String), CompressedDictionaryElementWrapper>,
}

fn lowercase_with_first_uppercase(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            first.to_uppercase().collect::<String>() + chars.as_str().to_lowercase().as_str()
        }
    }
}

impl DictionaryStore {
    pub fn from_elements_dump(path: &String) -> std::io::Result<Self> {
        let start_t = Instant::now();

        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        let elements: Vec<CompressedDictionaryElementWrapper> =
            bincode::deserialize(&buffer).unwrap();
        let e_c = elements.len();
        let store: DashMap<(TargetLanguage, String), CompressedDictionaryElementWrapper> =
            DashMap::new();

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

        Ok(Self { datastore: store })
    }

    pub fn query(&self, lang: TargetLanguage, word: &str) -> Option<DictionaryElementData> {
        let key = (lang.clone(), word.to_string());
        println!("Querying with key: {:?}", key);

        // Try querying with the original word
        if let Some(compressed_wrapper) = self.datastore.get(&key) {
            println!("Found value for original word");
            return Some(self.decompress_element(compressed_wrapper.value()));
        }

        let all_lowercase = word.to_lowercase();

        // If not found and the word isn't all lowercase, try again with the lowercase word
        if word != all_lowercase {
            let lower_key = (lang.clone(), all_lowercase);
            println!("Trying lowercase key: {:?}", lower_key);
            if let Some(compressed_wrapper) = self.datastore.get(&lower_key) {
                println!("Found value for lowercase word");
                return Some(self.decompress_element(compressed_wrapper.value()));
            }
        }

        // now try all lowercase with the first character uppercase
        let with_first = lowercase_with_first_uppercase(word);
        if with_first != word {
            let with_key = (lang, with_first);
            println!("Trying with lowercase-except-first key: {:?}", with_key);
            if let Some(compressed_wrapper) = self.datastore.get(&with_key) {
                println!("Found value for with-first word");
                return Some(self.decompress_element(compressed_wrapper.value()));
            }
        }

        println!("No value found for word: {}", word);
        None
    }

    fn decompress_element(
        &self,
        compressed: &CompressedDictionaryElementWrapper,
    ) -> DictionaryElementData {
        let decompressed_data: DictionaryElementData =
            bincode::deserialize(&decode_all(&compressed.compressed_data[..]).unwrap()).unwrap();

        DictionaryElementData {
            word: compressed.word.clone(),
            lang: compressed.lang.clone(),
            audio: decompressed_data.audio,
            ipa: decompressed_data.ipa,
            word_types: decompressed_data.word_types,
            definitions: decompressed_data.definitions,
            translation: decompressed_data.translation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_word() {
        let path = "../build_dump/compressed_dict.bin".to_string();
        println!("Initing store");

        let store = DictionaryStore::from_elements_dump(&path).unwrap();

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
