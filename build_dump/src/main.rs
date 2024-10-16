use memmap2::Mmap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

mod phase1;
mod phase2;
mod phase3;
mod phase4;

use libdictdefinition::{CompressedDictionaryElementWrapper, Definition, DictionaryElementData};

use Languages::TargetLanguage;

use phase1::build_word_set;
use phase2::build_dictionary_data;
use phase3::compress_dictionary_data;
use phase4::output_compressed_dict;

fn main() -> std::io::Result<()> {
    let input_path = Path::new("../raw-wiktextract-data.jsonl");
    let output_path = Path::new("./compressed_dict.bin");
    let json_output_path = Path::new("./uncompressed_dict.json");

    // Phase 1: Build a set of (word, language) combinations
    let word_set = build_word_set(input_path)?;
    println!("Phase 1 complete. Word set size: {}", word_set.len());

    // Phase 2: Build the dictionary data
    let dictionary_data = build_dictionary_data(input_path, &word_set)?;
    println!(
        "Phase 2 complete. Dictionary data size: {}",
        dictionary_data.len()
    );

    // Output JSON sample for the German word "Haus"
    output_json_sample(
        &dictionary_data,
        "Haus",
        TargetLanguage::German,
        json_output_path,
    )?;

    // Phase 3: Compress the dictionary data
    let compressed_data = compress_dictionary_data(dictionary_data);
    println!(
        "Phase 3 complete. Compressed data size: {}",
        compressed_data.len()
    );

    // Phase 4: Output to compressed_dict.bin file
    output_compressed_dict(&compressed_data, output_path)?;
    println!("Phase 4 complete. Output written to {:?}", output_path);

    Ok(())
}

fn output_json_sample(
    dictionary_data: &[DictionaryElementData],
    word: &str,
    lang: TargetLanguage,
    output_path: &Path,
) -> std::io::Result<()> {
    let sample = dictionary_data
        .iter()
        .filter(|e| e.word == word && e.lang == lang)
        .collect::<Vec<_>>();

    let json_file = File::create(output_path)?;
    serde_json::to_writer_pretty(json_file, &sample)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    Ok(())
}
