use memmap2::Mmap;
use rayon::prelude::*;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use Languages::TargetLanguage;

const BATCH_SIZE: usize = 12 * 1000;

pub fn build_word_set(input_path: &Path) -> std::io::Result<HashSet<(String, TargetLanguage)>> {
    let file = File::open(input_path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let mut reader = BufReader::new(&*mmap);
    let mut word_set = HashSet::new();
    let mut batch = Vec::with_capacity(BATCH_SIZE);
    let mut total_processed = 0;
    let mut last_print = 0;

    loop {
        batch.clear();
        for _ in 0..BATCH_SIZE {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => batch.push(line),
                Err(e) => return Err(e),
            }
        }

        if batch.is_empty() {
            break;
        }

        // CORRECTED LOGIC: Use flat_map to handle one-to-many language mappings.
        let batch_results: HashSet<(String, TargetLanguage)> = batch
            .par_iter()
            .flat_map(|line| {
                // This closure now returns a Vec of entries, which flat_map will flatten.
                let mut entries = Vec::new();
                if let Ok(json) = serde_json::from_str::<Value>(line) {
                    if let (Some(word), Some(lang_code)) = (
                        json.get("word").and_then(Value::as_str),
                        json.get("lang_code").and_then(Value::as_str),
                    ) {
                        let languages = TargetLanguage::from_wiktionary_language_code_n(lang_code);
                        for lang in languages {
                            entries.push((word.to_string(), lang));
                        }
                    }
                }
                entries
            })
            .collect();

        word_set.extend(batch_results);
        total_processed += batch.len();

        if total_processed - last_print > 100000 {
            println!("Processed {} entries for word set", total_processed);
            last_print = total_processed;
        }
    }

    println!("Word set built with {} entries", word_set.len());
    Ok(word_set)
}
