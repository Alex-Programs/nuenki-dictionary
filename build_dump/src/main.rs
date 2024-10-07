use memmap2::Mmap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Write};
use std::path::Path;
use zstd::stream::{decode_all, encode_all};
use Languages::TargetLanguage;

const COMPRESS_LVL: i32 = 9;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CompressedDictionaryElementWrapper {
    word: String,
    lang: TargetLanguage,
    compressed_data: Vec<u8>, // Compressed blob of DictionaryElementData
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DictionaryElementData {
    audio: Vec<String>,
    ipa: Option<String>,
    word_types: Vec<String>,
    definitions: Vec<Definition>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
struct Definition {
    text: String,
    tags: Vec<String>,
}

fn main() -> std::io::Result<()> {
    let input_path = Path::new("../raw-wiktextract-data.jsonl");
    let output_path = Path::new("./compressed_dict.bin");
    let json_output_path = Path::new("./compressed_dict.json");

    let file = File::open(input_path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let mut reader = BufReader::new(&*mmap);
    let mut batch = Vec::with_capacity(3000);

    let mut c = 0;
    let mut c_a = 0;
    let mut c_dd = 0;

    let mut last_print = 0;
    let mut out_vec: Vec<CompressedDictionaryElementWrapper> = Vec::with_capacity(3000);

    loop {
        batch.clear();
        for _ in 0..3000 {
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

        let results: Vec<CompressedDictionaryElementWrapper> = batch
            .par_iter()
            .filter_map(|line| process_element(line.as_str()))
            .collect();

        c_a += results.len();
        c += batch.len();

        let merged_results = merge_duplicates(results);

        c_dd += merged_results.len();

        out_vec.extend(merged_results);

        if c_a - last_print > 10000 {
            let ratio = (c_a as f64 / c as f64) * 100.0;
            println!("{} | {} {} {:.3}%", c_dd, c_a, c, ratio);
            last_print = c_a;
        }
    }

    let encoded: Vec<u8> = bincode::serialize(&out_vec).unwrap();
    let mut file = File::create(output_path)?;
    file.write_all(&encoded)?;

    let haus_entries: Vec<&CompressedDictionaryElementWrapper> =
        out_vec.iter().filter(|e| e.word == "Haus").collect();

    let json_file = File::create(json_output_path)?;
    serde_json::to_writer_pretty(json_file, &haus_entries)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    Ok(())
}

fn merge_duplicates(
    elements: Vec<CompressedDictionaryElementWrapper>,
) -> Vec<CompressedDictionaryElementWrapper> {
    let mut word_map: HashMap<(String, TargetLanguage), CompressedDictionaryElementWrapper> =
        HashMap::new();

    for element in elements {
        let key = (element.word.clone(), element.lang.clone());
        word_map
            .entry(key)
            .and_modify(|existing| {
                let mut existing_data: DictionaryElementData =
                    bincode::deserialize(&decode_all(&existing.compressed_data[..]).unwrap())
                        .unwrap();
                let new_data: DictionaryElementData =
                    bincode::deserialize(&decode_all(&element.compressed_data[..]).unwrap())
                        .unwrap();

                existing_data.audio.extend(new_data.audio);
                existing_data.audio.dedup();

                if existing_data.ipa.is_none() {
                    existing_data.ipa = new_data.ipa;
                }

                existing_data.word_types.extend(new_data.word_types);
                existing_data.word_types.dedup();

                existing_data.definitions.extend(new_data.definitions);
                existing_data.definitions.dedup();

                existing.compressed_data = encode_all(
                    &bincode::serialize(&existing_data).unwrap()[..],
                    COMPRESS_LVL,
                )
                .unwrap();
            })
            .or_insert(element);
    }

    word_map.into_values().collect()
}

fn process_element(text: &str) -> Option<CompressedDictionaryElementWrapper> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;

    let language = get_language(&json)?;
    let word = get_word(&json)?;
    let ipa = get_ipa(&json);
    let audio = get_audio(&json);
    let word_types = get_word_types(&json)?;
    let definitions = get_definitions(&json)?;

    let data = DictionaryElementData {
        audio,
        ipa,
        word_types,
        definitions,
    };

    let encoded = bincode::serialize(&data).unwrap();

    let readable = Cursor::new(encoded);

    let compressed_data = encode_all(readable, COMPRESS_LVL).unwrap();

    Some(CompressedDictionaryElementWrapper {
        word,
        lang: language,
        compressed_data,
    })
}

fn get_word(json: &serde_json::Value) -> Option<String> {
    json.get("word")
        .and_then(|word| word.as_str().map(|s| s.to_string()))
}

fn get_language(json: &serde_json::Value) -> Option<TargetLanguage> {
    let language_code = json.get("lang_code")?.as_str()?;
    TargetLanguage::from_wiktionary_language_code(language_code)
}

fn get_ipa(json: &serde_json::Value) -> Option<String> {
    json.get("sounds")?
        .as_array()?
        .iter()
        .filter_map(|sound| sound.get("ipa").and_then(|ipa| ipa.as_str()))
        .next() // Take the first IPA if there are multiple
        .map(|s| s.to_string())
}

fn get_audio(json: &serde_json::Value) -> Vec<String> {
    json.get("sounds")
        .and_then(|sounds| sounds.as_array())
        .map_or(Vec::new(), |sounds| {
            sounds
                .iter()
                .filter_map(|sound| {
                    sound
                        .get("ogg_url")
                        .or_else(|| sound.get("mp3_url"))
                        .and_then(|url| url.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        })
}

fn get_word_types(json: &serde_json::Value) -> Option<Vec<String>> {
    json.get("pos")
        .and_then(|pos| pos.as_str())
        .map(|s| vec![s.to_string()])
        .or_else(|| {
            json.get("head_templates")
                .and_then(|templates| templates.as_array())
                .map(|templates| {
                    templates
                        .iter()
                        .filter_map(|template| {
                            template
                                .get("name")
                                .and_then(|name| name.as_str())
                                .map(|s| s.to_string())
                        })
                        .collect()
                })
        })
}

fn get_definitions(json: &serde_json::Value) -> Option<Vec<Definition>> {
    json.get("senses")
        .and_then(|senses| senses.as_array())
        .map(|senses| {
            senses
                .iter()
                .filter_map(|sense| {
                    let tags = sense.get("tags").and_then(|t| t.as_array()).map_or(
                        Vec::new(),
                        |tag_array| {
                            tag_array
                                .iter()
                                .filter_map(|tag| tag.as_str())
                                .map(|s| s.to_string())
                                .collect()
                        },
                    );

                    sense
                        .get("glosses")
                        .and_then(|g| g.as_array())
                        .and_then(|glosses| {
                            glosses.first().and_then(|def| {
                                def.as_str().map(|def_str| Definition {
                                    text: def_str.to_string(),
                                    tags: tags.clone(),
                                })
                            })
                        })
                })
                .collect()
        })
}
