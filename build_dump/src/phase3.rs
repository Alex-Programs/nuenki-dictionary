use rayon::prelude::*;
use zstd::stream::encode_all;

use libdictdefinition::{CompressedDictionaryElementWrapper, DictionaryElementData};

const COMPRESS_LVL: i32 = 4;
const BATCH_SIZE: usize = 12 * 1000 * 2;

pub fn compress_dictionary_data(
    dictionary_data: Vec<DictionaryElementData>,
) -> Vec<CompressedDictionaryElementWrapper> {
    let total_elements = dictionary_data.len();
    let mut compressed_data = Vec::with_capacity(total_elements);
    let mut processed = 0;

    for chunk in dictionary_data.chunks(BATCH_SIZE) {
        let batch_results: Vec<CompressedDictionaryElementWrapper> = chunk
            .par_iter()
            .map(|element| {
                let encoded = bincode::serialize(&element).unwrap();
                let compressed = encode_all(&encoded[..], COMPRESS_LVL).unwrap();

                CompressedDictionaryElementWrapper {
                    word: element.word.clone(),
                    lang: element.lang.clone(),
                    compressed_data: compressed,
                }
            })
            .collect();

        compressed_data.extend(batch_results);
        processed += chunk.len();

        println!(
            "Compressed {}/{} entries ({:.2}%)",
            processed,
            total_elements,
            (processed as f64 / total_elements as f64) * 100.0
        );
    }

    println!("Compression completed for all {} entries", total_elements);
    compressed_data
}
