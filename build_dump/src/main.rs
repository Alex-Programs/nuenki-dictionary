use memmap2::Mmap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use Languages::TargetLanguage;

extern crate savefile;
use savefile::prelude::*;

#[macro_use]
extern crate savefile_derive;

#[derive(Clone, Debug, Savefile, Serialize)]
struct DictionaryElement {
    word: String,
    lang: TargetLanguage,
    audio: Vec<String>,
    ipa: Option<String>,
    word_types: Vec<String>,
    definitions: Vec<Definition>,
}

#[derive(Clone, Debug, Savefile, Serialize, PartialEq, Eq, Hash)]
struct Definition {
    text: String,
    tags: Vec<String>,
}

fn main() -> std::io::Result<()> {
    // Open the file for reading
    let file = File::open("../raw-wiktextract-data.jsonl")?;

    // Create a memory-mapped view of the file
    let mmap = unsafe { Mmap::map(&file)? };

    let mut reader = BufReader::new(&*mmap);
    let mut batch = Vec::with_capacity(3000);

    let mut c = 0;
    let mut c_a = 0;
    let mut c_dd = 0;

    let mut last_print = 0;
    let mut out_vec: Vec<DictionaryElement> = Vec::with_capacity(3000);

    loop {
        batch.clear();
        for _ in 0..3000 {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break, // End of file
                Ok(_) => batch.push(line),
                Err(e) => return Err(e), // Handle read error
            }
        }

        if batch.is_empty() {
            break; // End of file
        }

        let results: Vec<DictionaryElement> = batch
            .par_iter()
            .filter_map(|line| process_element(line.as_str()))
            .collect();

        c_a += results.len();
        c += batch.len();

        // Merge duplicates
        let merged_results = merge_duplicates(results);

        c_dd += merged_results.len();

        out_vec.extend(merged_results);

        if c_a - last_print > 32000 {
            let ratio = (c_a as f64 / c as f64) * 100.0;
            println!("{} | {} {} {:.3}%", c_dd, c_a, c, ratio);
            last_print = c_a;
        }
    }

    save_file("./dict.bin", 0, &out_vec).unwrap();

    // Dump to a JSON, but only entries where the word is "Haus"
    let haus_entries: Vec<&DictionaryElement> =
        out_vec.iter().filter(|e| e.word == "Haus").collect();

    let json_file = File::create("./dict.json")?;
    serde_json::to_writer_pretty(json_file, &haus_entries)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    Ok(())
}

fn merge_duplicates(elements: Vec<DictionaryElement>) -> Vec<DictionaryElement> {
    let mut word_map: HashMap<(String, TargetLanguage), DictionaryElement> = HashMap::new();

    for element in elements {
        let key = (element.word.clone(), element.lang.clone());
        word_map
            .entry(key)
            .and_modify(|existing| {
                // Merge audio
                existing.audio.extend(element.audio.clone());
                existing.audio.dedup();

                // Merge IPA (keep the first non-None value)
                if existing.ipa.is_none() {
                    existing.ipa = element.ipa.clone();
                }

                // Merge word types
                existing.word_types.extend(element.word_types.clone());
                existing.word_types.dedup();

                // Merge definitions
                existing.definitions.extend(element.definitions.clone());
                existing.definitions.dedup();
            })
            .or_insert(element);
    }

    word_map.into_values().collect()
}

fn process_element(text: &str) -> Option<DictionaryElement> {
    let json: serde_json::Value = match serde_json::from_str(text) {
        Ok(d) => d,
        Err(e) => {
            println!("Error parsing: {:?}", e);
            return None;
        }
    };

    let language = get_language(&json)?;

    let word = get_word(&json)?;

    let ipa = get_ipa(&json);

    let audio = get_audio(&json);

    let word_types = get_word_types(&json)?;

    let definitions = get_definitions(&json)?;

    Some(DictionaryElement {
        word,
        lang: language,
        audio,
        ipa,
        word_types,
        definitions,
    })
}

fn get_word(json: &serde_json::Value) -> Option<String> {
    match json.get("word") {
        Some(word) => {
            let r = Some(word.as_str().unwrap().to_string());
            //println!("Returning {:?}", r);

            r
        }
        None => None,
    }
}

fn get_definitions(json: &serde_json::Value) -> Option<Vec<Definition>> {
    let senses = json.get("senses")?.as_array()?;

    let mut definitions = Vec::new();

    for sense in senses {
        let tags = sense
            .get("tags")
            .and_then(|t| t.as_array())
            .map_or(Vec::new(), |tag_array| {
                tag_array
                    .iter()
                    .filter_map(|tag| tag.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            });

        // Get definitions from glosses only
        if let Some(glosses) = sense.get("glosses").and_then(|g| g.as_array()) {
            for def in glosses {
                if let Some(def_str) = def.as_str() {
                    definitions.push(Definition {
                        text: def_str.to_string(),
                        tags: tags.clone(), // Use the collected tags
                    });
                }
            }
        }
    }

    Some(definitions)
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
            let mut audios = Vec::new();
            for sound in sounds {
                if let Some(ogg_url) = sound.get("ogg_url").and_then(|url| url.as_str()) {
                    audios.push(ogg_url.to_string());
                } else if let Some(mp3_url) = sound.get("mp3_url").and_then(|url| url.as_str()) {
                    audios.push(mp3_url.to_string());
                }
            }
            audios
        })
}

fn get_word_types(json: &serde_json::Value) -> Option<Vec<String>> {
    if let Some(pos) = json.get("pos").and_then(|p| p.as_str()) {
        return Some(vec![pos.to_string()]); // Basic support for a single word type
    }

    json.get("head_templates")?.as_array().map(|templates| {
        templates
            .iter()
            .filter_map(|template| template.get("name").and_then(|name| name.as_str()))
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    })
}
