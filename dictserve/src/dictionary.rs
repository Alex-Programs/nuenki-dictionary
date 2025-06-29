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
include!(concat!(env!("OUT_DIR"), "/czech_lemmas.rs"));

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

fn lemmatize_czech(word: &str) -> String {
    CZECH_LEMMAS
        .get(word)
        .map(|&lemma| lemma.to_string())
        .unwrap_or_else(|| word.to_string())
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
        // Try querying with the original key
        let search_key = (lang.clone(), key.to_string());
        if let Some(compressed_wrapper) = self.datastore.get(&search_key) {
            return Some(self.decompress_element(compressed_wrapper.value()));
        }

        // If not found, try again with the all-lowercase key
        let all_lowercase = key.to_lowercase();
        if key != all_lowercase {
            let lower_key = (lang.clone(), all_lowercase.clone());
            if let Some(compressed_wrapper) = self.datastore.get(&lower_key) {
                return Some(self.decompress_element(compressed_wrapper.value()));
            }
        }

        // Now try all lowercase with the first character uppercase
        let with_first = lowercase_with_first_uppercase(key);
        if with_first != key {
            let with_key = (lang.clone(), with_first);
            if let Some(compressed_wrapper) = self.datastore.get(&with_key) {
                return Some(self.decompress_element(compressed_wrapper.value()));
            }
        }

        // --- New Czech-specific logic ---
        // If all else fails, and the language is Czech, try stripping declensions.
        if lang == TargetLanguage::Czech {
            let stripped_key_slice = lemmatize_czech(key);

            // Only proceed if stripping the declension actually changed the key.
            if stripped_key_slice != key {
                // As per request, first try the stripped word in UPPERCASE.
                // Many Czech nouns are stored as uppercase entries.
                let stripped_upper = stripped_key_slice.to_uppercase();
                let stripped_upper_key = (lang.clone(), stripped_upper);
                if let Some(compressed_wrapper) = self.datastore.get(&stripped_upper_key) {
                    return Some(self.decompress_element(compressed_wrapper.value()));
                }

                // If that fails, try the stripped word in lowercase.
                let stripped_lower = stripped_key_slice.to_lowercase();
                if stripped_lower != stripped_key_slice {
                    // Avoid re-querying if it's already lowercase
                    let stripped_lower_key = (lang.clone(), stripped_lower);
                    if let Some(compressed_wrapper) = self.datastore.get(&stripped_lower_key) {
                        return Some(self.decompress_element(compressed_wrapper.value()));
                    }
                }
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
    fn test_lemmatize_known_words() {
        assert_eq!(lemmatize_czech("Aachenu"), "Aachen");
        assert_eq!(lemmatize_czech("abecedu"), "abeceda");
        assert_eq!(lemmatize_czech("absentovala"), "absentovat");
    }

    #[test]
    fn test_lemmatize_preserves_case() {
        assert_eq!(lemmatize_czech("Abrahámu"), "Abrahám");
    }
}
