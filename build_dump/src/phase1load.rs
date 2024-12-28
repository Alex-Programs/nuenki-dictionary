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

        let batch_results: HashSet<(String, TargetLanguage)> = batch
            .par_iter()
            .filter_map(|line| {
                let json: Value = serde_json::from_str(line).ok()?;
                let word = json.get("word")?.as_str()?.to_string();
                let lang_code = json.get("lang_code")?.as_str()?;
                let language = TargetLanguage::from_wiktionary_language_code_n(lang_code)?;
                Some((word, language))
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
