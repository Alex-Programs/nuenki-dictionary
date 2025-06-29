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
            info!("Example entry: {:?}", element);
            store.insert((element.lang.clone(), element.key.clone()), element);
        });

        let time_taken = start_t.elapsed();
        info!(
            "Loaded {} items from elements dump in {}s",
            e_c,
            time_taken.as_secs_f32()
        );

        Ok(Self { datastore: store })
    }

    pub fn query(&self, lang: TargetLanguage, key: &str) -> Option<DictionaryElementData> {
        let search_key = (lang.clone(), key.to_string());

        // Try querying with the original key
        if let Some(compressed_wrapper) = self.datastore.get(&search_key) {
            return Some(self.decompress_element(compressed_wrapper.value()));
        }

        let all_lowercase = key.to_lowercase();

        // If not found and the key isn't all lowercase, try again with the lowercase key
        if key != all_lowercase {
            let lower_key = (lang.clone(), all_lowercase);
            if let Some(compressed_wrapper) = self.datastore.get(&lower_key) {
                return Some(self.decompress_element(compressed_wrapper.value()));
            }
        }

        // now try all lowercase with the first character uppercase
        let with_first = lowercase_with_first_uppercase(key);
        if with_first != key {
            let with_key = (lang, with_first);
            if let Some(compressed_wrapper) = self.datastore.get(&with_key) {
                return Some(self.decompress_element(compressed_wrapper.value()));
            }
        }

        None
    }

    fn decompress_element(
        &self,
        compressed: &CompressedDictionaryElementWrapper,
    ) -> DictionaryElementData {
        let decompressed_data: DictionaryElementData =
            bincode::deserialize(&decode_all(&compressed.compressed_data[..]).unwrap()).unwrap();

        decompressed_data
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
