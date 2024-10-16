use memmap2::Mmap;
use rayon::prelude::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use libdictdefinition::{Definition, DictionaryElementData, HyperlinkedText};

use Languages::TargetLanguage;

const BATCH_SIZE: usize = 12 * 1000;

pub fn build_dictionary_data(
    input_path: &Path,
    word_set: &HashSet<(String, TargetLanguage)>,
) -> std::io::Result<Vec<DictionaryElementData>> {
    let file = File::open(input_path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let mut reader = BufReader::new(&*mmap);
    let mut dictionary_data = Vec::new();
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

        let batch_results: Vec<DictionaryElementData> = batch
            .par_iter()
            .filter_map(|line| {
                let json: Value = serde_json::from_str(line).ok()?;
                process_json_entry(&json, word_set)
            })
            .collect();

        dictionary_data.extend(batch_results);
        total_processed += batch.len();

        if total_processed - last_print > 100000 {
            println!("Processed {} entries for dictionary data", total_processed);
            last_print = total_processed;
        }
    }

    println!(
        "Dictionary data built with {} entries, Now to merge...",
        dictionary_data.len()
    );

    let dictionary_data = merge_duplicates(dictionary_data);

    println!("Merge complete");

    Ok(dictionary_data)
}

fn merge_duplicates(elements: Vec<DictionaryElementData>) -> Vec<DictionaryElementData> {
    let mut word_map: HashMap<(String, TargetLanguage), DictionaryElementData> = HashMap::new();

    for element in elements {
        let key = (element.word.clone(), element.lang.clone());
        word_map
            .entry(key)
            .and_modify(|existing| {
                // Merge audio
                existing.audio.extend(element.audio.clone());
                dedup_preserve_order(&mut existing.audio);

                // Merge IPA
                if existing.ipa.is_none() {
                    existing.ipa = element.ipa.clone();
                }

                // Merge word types
                existing.word_types.extend(element.word_types.clone());
                dedup_preserve_order(&mut existing.word_types);

                // Merge definitions
                existing.definitions.extend(element.definitions.clone());
                dedup_preserve_order(&mut existing.definitions);

                // Merge translations
                if existing.translation.is_none() {
                    existing.translation = element.translation.clone();
                }
            })
            .or_insert(element);
    }

    word_map.into_values().collect()
}

fn dedup_preserve_order<T: Eq + std::hash::Hash + Clone>(v: &mut Vec<T>) {
    let mut seen = std::collections::HashSet::new();
    v.retain(|item| seen.insert(item.clone()));
}

fn process_json_entry(
    json: &Value,
    word_set: &HashSet<(String, TargetLanguage)>,
) -> Option<DictionaryElementData> {
    let word = json.get("word")?.as_str()?.to_string();
    let lang_code = json.get("lang_code")?.as_str()?;
    let language = TargetLanguage::from_wiktionary_language_code(lang_code)?;

    if !word_set.contains(&(word.clone(), language.clone())) {
        return None;
    }

    let audio = get_audio(json);
    let ipa = get_ipa(json);
    let word_types = get_word_types(json)?;
    let definitions = get_definitions(json, word_set, &language)?;
    let translation = get_english_translation(json);

    Some(DictionaryElementData {
        word,
        lang: language,
        audio,
        ipa,
        word_types,
        definitions,
        translation: translation,
    })
}

fn get_audio(json: &Value) -> Vec<String> {
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

fn get_ipa(json: &Value) -> Option<String> {
    json.get("sounds")?
        .as_array()?
        .iter()
        .filter_map(|sound| sound.get("ipa").and_then(|ipa| ipa.as_str()))
        .next()
        .map(|s| s.to_string())
}

fn get_word_types(json: &Value) -> Option<Vec<String>> {
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

const FILTER_TAGS: [&'static str; 20] = [
    "class-1",
    "class-2",
    "class-3",
    "class-4",
    "class-5",
    "class-6",
    "class-7",
    "declension-1",
    "declension-2",
    "declension-3",
    "declension-4",
    "declension-5",
    "conjugation-1",
    "conjugation-2",
    "conjugation-3",
    "conjugation-4",
    "stress-pattern-1",
    "stress-pattern-2",
    "stress-pattern-3",
    "stress-pattern-4",
];

fn uppercase_first_character_latin(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }

    let first_char = text.chars().next().unwrap();
    if first_char.is_ascii_lowercase() {
        let upper_char = first_char.to_ascii_uppercase();
        return upper_char.to_string() + &text[1..];
    }

    text.to_string()
}

fn get_definitions(
    json: &Value,
    word_set: &HashSet<(String, TargetLanguage)>,
    language: &TargetLanguage,
) -> Option<Vec<Definition>> {
    let mut out = Vec::new();

    let senses = json.get("senses").and_then(|senses| senses.as_array())?;

    for sense in senses {
        let tags = sense
            .get("tags")
            .and_then(|t| t.as_array())
            .map_or(Vec::new(), |tag_array| {
                tag_array
                    .iter()
                    .filter_map(|tag| tag.as_str())
                    .filter(|t| !FILTER_TAGS.contains(t))
                    .map(|s| uppercase_first_character_latin(s)) // makes it into a String as a bonus
                    .collect()
            });

        let gloss = sense
            .get("glosses")
            .and_then(|g| g.as_array())
            .and_then(|g| g.first());

        match gloss {
            None => {
                continue;
            }
            Some(g) => {
                let as_str = g.as_str();
                let as_str = match as_str {
                    Some(t) => t,
                    None => continue,
                };
                let as_string = as_str.to_string();

                out.push(Definition {
                    text: hyperlink_text(as_string, &word_set, &language),
                    tags,
                })
            }
        }
    }

    Some(out)
}

fn hyperlink_text(
    text: String,
    word_set: &HashSet<(String, TargetLanguage)>,
    language: &TargetLanguage,
) -> Vec<HyperlinkedText> {
    let mut result = Vec::new();
    let mut current_word = String::new();
    let mut was_last_whitespace = false;

    fn is_non_content(c: &char) -> bool {
        let also_prohibited = [
            '!', '"', '£', '$', '%', '^', '&', '*', '(', ')', '-', '_', '=', '+', '[', ']', ':',
            ';', '\'', '~', '@', '#', '<', ',', '.', '>', '/', '?', '\\', '|',
        ];

        c.is_whitespace() || c.is_numeric() || also_prohibited.contains(c)
    }

    for c in text.chars() {
        if is_non_content(&c) {
            if was_last_whitespace {
                current_word.push(c);
            } else {
                if !current_word.is_empty() {
                    if word_set.contains(&(current_word.clone(), language.clone())) {
                        result.push(HyperlinkedText::Link(current_word.clone()));
                    } else {
                        result.push(HyperlinkedText::Plain(current_word.clone()));
                    }
                    current_word.clear();

                    current_word.push(c);
                }
            }

            was_last_whitespace = true;
        } else {
            if was_last_whitespace {
                result.push(HyperlinkedText::Plain(current_word.clone()));
                current_word.clear();
                current_word.push(c);
            } else {
                current_word.push(c);
            }

            was_last_whitespace = false;
        }
    }

    // Handle the last word if there's no trailing whitespace
    if !current_word.is_empty() {
        if word_set.contains(&(current_word.clone(), language.clone())) {
            result.push(HyperlinkedText::Link(current_word));
        } else {
            result.push(HyperlinkedText::Plain(current_word));
        }
    }

    result
}

fn get_english_translation(json: &Value) -> Option<String> {
    if json.get("translations").is_some() {
        println!("Translations raw: {:?}", json.get("translations").unwrap());
    }

    let translations = json
        .get("translations")
        .and_then(|translations| translations.as_array())?;

    println!("Translations: {:?}", translations);

    translations
        .iter()
        .filter_map(|translation| {
            let lang_code = translation.get("code")?.as_str()?;
            println!("Lang code: {}", lang_code);

            match lang_code {
                "en" => Some(translation.get("word")?.to_string()),
                _ => None,
            }
        })
        .next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_hyperlink_single_word() {
        let mut word_set = HashSet::new();
        word_set.insert(("bonjour".to_string(), TargetLanguage::French));

        let input = "bonjour".to_string();
        let expected = vec![HyperlinkedText::Link("bonjour".to_string())];
        assert_eq!(
            hyperlink_text(input, &word_set, &TargetLanguage::French),
            expected
        );
    }

    #[test]
    fn test_plain_single_word() {
        let word_set = HashSet::new(); // No words in set

        let input = "hallo".to_string();
        let expected = vec![HyperlinkedText::Plain("hallo".to_string())];
        assert_eq!(
            hyperlink_text(input, &word_set, &TargetLanguage::German),
            expected
        );
    }

    #[test]
    fn test_mixed_words() {
        let mut word_set = HashSet::new();
        word_set.insert(("bonjour".to_string(), TargetLanguage::French));

        let input = "bonjour hallo".to_string();
        let expected = vec![
            HyperlinkedText::Link("bonjour".to_string()),
            HyperlinkedText::Plain(" ".to_string()), // Space preserved
            HyperlinkedText::Plain("hallo".to_string()),
        ];
        assert_eq!(
            hyperlink_text(input, &word_set, &TargetLanguage::French),
            expected
        );
    }

    #[test]
    fn test_multiple_spaces_between_words() {
        let mut word_set = HashSet::new();
        word_set.insert(("bonjour".to_string(), TargetLanguage::French));

        let input = "bonjour   hallo".to_string();
        let expected = vec![
            HyperlinkedText::Link("bonjour".to_string()),
            HyperlinkedText::Plain("   ".to_string()), // Multiple spaces preserved
            HyperlinkedText::Plain("hallo".to_string()),
        ];
        assert_eq!(
            hyperlink_text(input, &word_set, &TargetLanguage::French),
            expected
        );
    }

    #[test]
    fn test_non_space_whitespace() {
        let mut word_set = HashSet::new();
        word_set.insert(("bonjour".to_string(), TargetLanguage::French));

        let input = "bonjour\tguten".to_string(); // Tab character
        let expected = vec![
            HyperlinkedText::Link("bonjour".to_string()),
            HyperlinkedText::Plain("\t".to_string()), // Tab preserved
            HyperlinkedText::Plain("guten".to_string()),
        ];
        assert_eq!(
            hyperlink_text(input, &word_set, &TargetLanguage::French),
            expected
        );
    }

    #[test]
    fn test_hyperlink_in_different_language() {
        let mut word_set = HashSet::new();
        word_set.insert(("guten".to_string(), TargetLanguage::German));

        let input = "guten bonjour".to_string();
        let expected = vec![
            HyperlinkedText::Link("guten".to_string()),
            HyperlinkedText::Plain(" ".to_string()), // Space preserved
            HyperlinkedText::Plain("bonjour".to_string()),
        ];
        assert_eq!(
            hyperlink_text(input, &word_set, &TargetLanguage::German),
            expected
        );
    }

    #[test]
    fn test_hyperlink_in_mixed_language() {
        let mut word_set = HashSet::new();
        word_set.insert(("bonjour".to_string(), TargetLanguage::French));
        word_set.insert(("guten".to_string(), TargetLanguage::German));

        let input = "guten bonjour".to_string();

        assert_eq!(
            hyperlink_text(input.clone(), &word_set, &TargetLanguage::German),
            vec![
                HyperlinkedText::Link("guten".to_string()),
                HyperlinkedText::Plain(" ".to_string()),
                HyperlinkedText::Plain("bonjour".to_string()),
            ]
        );
    }

    #[test]
    fn test_empty_input() {
        let word_set = HashSet::new(); // No words in set

        let input = "".to_string(); // Empty input
        let expected: Vec<HyperlinkedText> = vec![];
        assert_eq!(
            hyperlink_text(input, &word_set, &TargetLanguage::German),
            expected
        );
    }
}
