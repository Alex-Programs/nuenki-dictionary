use memmap2::Mmap;
use rayon::prelude::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

use libdictdefinition::{Definition, DictionaryElementData, HyperlinkedText};

use Languages::TargetLanguage;

const BATCH_SIZE: usize = 12 * 1000;

pub fn build_dictionary_data(
    input_path: &Path,
    word_set: &HashSet<(String, TargetLanguage)>,
) -> std::io::Result<Vec<DictionaryElementData>> {
    let mut dictionary_data = Vec::new();
    let mut langs_set = Vec::new();

    // separate scope to encourage deallocation
    {
        let file = File::open(input_path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let mut reader = BufReader::new(&*mmap);
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

            // CORRECTED LOGIC: Use flat_map to create duplicate entries for each language.
            let batch_results: Vec<DictionaryElementData> = batch
                .par_iter()
                .flat_map(|line| {
                    if let Ok(json) = serde_json::from_str(line) {
                        process_json_entry(&json, word_set)
                    } else {
                        Vec::new()
                    }
                })
                .collect();

            for el in &batch_results {
                if !langs_set.contains(&el.lang) {
                    langs_set.push(el.lang.clone());
                    println!("Langs: {:?}", langs_set);
                }
            }

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
    }

    let dictionary_data = merge_duplicates(dictionary_data, langs_set);

    println!("Merge complete");

    Ok(dictionary_data)
}

fn merge_duplicates(
    mut elements: Vec<DictionaryElementData>,
    languages: Vec<TargetLanguage>,
) -> Vec<DictionaryElementData> {
    let mut result = Vec::new();

    for lang in languages {
        println!("Running merge on {:?}", lang);

        let mut word_map: HashMap<&String, DictionaryElementData> = HashMap::new();
        let mut to_remove = Vec::new();

        for (i, element) in elements.iter().enumerate() {
            if element.lang != lang {
                continue;
            }

            to_remove.push(i);

            word_map
                .entry(&element.word)
                .and_modify(|existing| {
                    existing.audio.extend(element.audio.clone());
                    dedup_preserve_order(&mut existing.audio);
                    if existing.ipa.is_none() {
                        existing.ipa = element.ipa.clone();
                    }
                    existing.word_types.extend(element.word_types.clone());
                    dedup_preserve_order(&mut existing.word_types);
                    existing.definitions.extend(element.definitions.clone());
                    consolidate_definitions(&mut existing.definitions);
                })
                .or_insert_with(|| {
                    let mut new_element = element.clone();
                    dedup_preserve_order(&mut new_element.audio);
                    dedup_preserve_order(&mut new_element.word_types);
                    consolidate_definitions(&mut new_element.definitions);
                    new_element
                });
        }

        println!("Done; extending");
        result.extend(word_map.into_values());

        println!("Now removing");
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for i in to_remove {
            elements.swap_remove(i);
        }
    }

    result
}

fn consolidate_definitions(existing_definitions: &mut Vec<Definition>) {
    let mut seen_texts = HashSet::new();
    let mut consolidated = Vec::new();

    for mut definition in existing_definitions.drain(..) {
        if seen_texts.insert(definition.text.clone()) {
            consolidated.push(definition);
        } else {
            if let Some(existing) = consolidated.iter_mut().find(|d| d.text == definition.text) {
                existing.tags.append(&mut definition.tags);
                dedup_preserve_order(&mut existing.tags);
            }
        }
    }

    *existing_definitions = consolidated;
}

fn dedup_preserve_order<T: Eq + std::hash::Hash + Clone>(v: &mut Vec<T>) {
    let mut seen = std::collections::HashSet::new();
    v.retain(|item| seen.insert(item.clone()));
}

// CORRECTED LOGIC: This function now returns a Vec of elements, one for each valid language.
fn process_json_entry(
    json: &Value,
    word_set: &HashSet<(String, TargetLanguage)>,
) -> Vec<DictionaryElementData> {
    let word = match json.get("word").and_then(Value::as_str) {
        Some(w) => w.to_string(),
        None => return Vec::new(),
    };
    let lang_code = match json.get("lang_code").and_then(Value::as_str) {
        Some(lc) => lc,
        None => return Vec::new(),
    };

    let languages = TargetLanguage::from_wiktionary_language_code_n(lang_code);
    if languages.is_empty() {
        return Vec::new();
    }

    // Parse common data once
    let audio = get_audio(json);
    let ipa = get_ipa(json);
    let word_types = match get_word_types(json) {
        Some(wt) => wt,
        None => return Vec::new(),
    };

    // Create a new Vec to hold the generated dictionary elements
    let mut results = Vec::new();

    for lang in languages {
        // Only create an entry if this specific (word, lang) pair is in our master set
        if word_set.contains(&(word.clone(), lang.clone())) {
            // The get_definitions call must be inside the loop because it depends on the language
            let definitions = match get_definitions(json, word_set, &lang) {
                Some(d) => d,
                None => continue, // Skip this language if it has no valid definitions
            };

            results.push(DictionaryElementData {
                key: word.clone(),
                word: word.clone(),
                lang: lang,
                audio: audio.clone(),
                ipa: ipa.clone(),
                word_types: word_types.clone(),
                definitions: definitions,
                dereferenced_text: None,
            });
        }
    }

    results
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
        let mut tags =
            sense
                .get("tags")
                .and_then(|t| t.as_array())
                .map_or(Vec::new(), |tag_array| {
                    tag_array
                        .iter()
                        .filter_map(|tag| tag.as_str())
                        .filter(|t| !FILTER_TAGS.contains(t))
                        .map(|s| uppercase_first_character_latin(s))
                        .collect()
                });
        tags.sort();

        let gloss = sense
            .get("glosses")
            .and_then(|g| g.as_array())
            .and_then(|g| g.first());

        if let Some(g) = gloss {
            if let Some(as_str) = g.as_str() {
                let as_string = solve_unopened_brackets(as_str.to_string());
                out.push(Definition {
                    text: hyperlink_text(as_string, &word_set, &language),
                    tags,
                })
            }
        }
    }

    Some(out)
}

fn solve_unopened_brackets(text: String) -> String {
    let bracket_pairs = [('(', ')'), ('[', ']'), ('{', '}')];
    let opening_brackets = bracket_pairs.map(|x| x.0);

    for char in text.chars() {
        if opening_brackets.contains(&char) {
            return text;
        }
        for (open, close) in bracket_pairs {
            if char == close {
                return format!("{}{}", open, text);
            }
        }
    }
    text
}

const WORD_SET_EXCEPTIONS: [&'static str; 63] = [
    "a", "A", "b", "B", "c", "C", "d", "D", "e", "E", "f", "F", "g", "G", "h", "H", "i", "I", "j",
    "J", "k", "K", "l", "L", "m", "M", "n", "N", "o", "O", "p", "P", "q", "Q", "r", "R", "s", "S",
    "t", "T", "u", "U", "v", "V", "w", "W", "x", "X", "y", "Y", "z", "Z", "0", "1", "2", "3", "4",
    "5", "6", "7", "8", "9", "not",
];

fn remove_diacritics(input: &str) -> String {
    input.nfd().filter(|&c| c != '\u{0301}').collect::<String>()
}

pub fn hyperlink_text(
    text: String,
    word_set: &HashSet<(String, TargetLanguage)>,
    language: &TargetLanguage,
) -> Vec<HyperlinkedText> {
    let mut result = Vec::new();
    let mut current_word = String::new();
    let mut was_last_filler = false;

    fn is_non_content(c: &char) -> bool {
        let also_prohibited = [
            '!', '"', '£', '$', '%', '^', '&', '*', '(', ')', '-', '_', '=', '+', '[', ']', ':',
            ';', '\'', '~', '@', '#', '<', ',', '.', '>', '/', '?', '\\', '|',
        ];
        c.is_whitespace() || c.is_numeric() || also_prohibited.contains(c)
    }

    let process_word = |word_str: &str| -> HyperlinkedText {
        if WORD_SET_EXCEPTIONS.contains(&word_str) {
            return HyperlinkedText::Plain(word_str.to_string());
        }

        if word_set.contains(&(word_str.to_string(), language.clone())) {
            return HyperlinkedText::Link(word_str.to_string());
        }

        if *language == TargetLanguage::Russian {
            let stripped = remove_diacritics(word_str);
            if stripped != word_str && word_set.contains(&(stripped.clone(), language.clone())) {
                return HyperlinkedText::Link(word_str.to_string());
            }
        }

        HyperlinkedText::Plain(word_str.to_string())
    };

    for c in text.chars() {
        if is_non_content(&c) {
            if was_last_filler {
                current_word.push(c);
            } else {
                if !current_word.is_empty() {
                    result.push(process_word(&current_word));
                    current_word.clear();
                }
                current_word.push(c);
            }
            was_last_filler = true;
        } else {
            if was_last_filler {
                result.push(HyperlinkedText::Plain(current_word.clone()));
                current_word.clear();
            }
            current_word.push(c);
            was_last_filler = false;
        }
    }

    if !current_word.is_empty() {
        if was_last_filler {
            result.push(HyperlinkedText::Plain(current_word));
        } else {
            result.push(process_word(&current_word));
        }
    }

    result
}

// Tests have been removed for brevity to avoid confusion. You can re-add them if needed.
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_remove_diacritics() {
        let cases = vec![
            ("ука́занный", "указанный"),
            ("Приве́т", "Привет"),
            ("йо́гурт", "йогурт"),
            ("те́ст", "тест"),
            ("й", "й"),
            ("hello", "hello"),
            ("", ""),
            ("123", "123"),
        ];

        for (input, expected) in cases {
            assert_eq!(remove_diacritics(input), expected, "Failed on: {}", input);
        }
    }

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
        let word_set = HashSet::new();
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
        assert_eq!(
            hyperlink_text(input, &word_set, &TargetLanguage::French),
            vec![
                HyperlinkedText::Link("bonjour".to_string()),
                HyperlinkedText::Plain(" ".to_string()),
                HyperlinkedText::Plain("hallo".to_string()),
            ]
        );
    }

    #[test]
    fn test_no_change() {
        assert_eq!(solve_unopened_brackets("()".to_string()), "()".to_string());
        assert_eq!(
            solve_unopened_brackets("[abc]".to_string()),
            "[abc]".to_string()
        );
    }

    #[test]
    fn test_add_opening_bracket() {
        assert_eq!(solve_unopened_brackets(")".to_string()), "()".to_string());
        assert_eq!(solve_unopened_brackets("]".to_string()), "[]".to_string());
        assert_eq!(solve_unopened_brackets("}".to_string()), "{}".to_string());
    }
}
