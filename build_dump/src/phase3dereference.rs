use libdictdefinition::{DictionaryElementData, HyperlinkedText};
use std::collections::{HashMap, HashSet};
use Languages::TargetLanguage;

pub fn process_dereferences(elements: Vec<DictionaryElementData>) -> Vec<DictionaryElementData> {
    let mut element_map: HashMap<(String, TargetLanguage), DictionaryElementData> = elements
        .into_iter()
        .map(|e| ((e.key.clone(), e.lang.clone()), e))
        .collect();

    let mut to_process = Vec::new();
    let mut to_process_keys = HashSet::new();

    // Identify elements to be dereferenced
    for ((key, lang), element) in &element_map {
        // Your intended heuristic: Only consider dereferencing if the entry doesn't have too many definitions.
        // Using 8 here, from your last provided code snippet.
        if element.definitions.len() > 6 {
            continue;
        }

        if let Some(first_def) = element.definitions.first() {
            // Corrected logic: only skip if NEITHER tag is present.
            if !first_def.tags.contains(&"Form-of".to_string())
                && !first_def.tags.contains(&"Alt-of".to_string())
            {
                continue;
            }

            // The new, robust parser will be called here.
            if let Some((dereferenced_text, referenced_word)) = parse_dereference(&first_def.text) {
                to_process.push((
                    key.clone(),
                    lang.clone(),
                    dereferenced_text,
                    referenced_word,
                ));
                to_process_keys.insert(key.clone());
            }
        }
    }

    // This crucial line from your 6-month backup prevents multi-stage dereferencing (e.g. A -> B -> C).
    to_process.retain(|x| !to_process_keys.contains(&x.3));

    // Perform dereferencing
    for (key, lang, dereferenced_text, referenced_word) in to_process {
        // Get the original element before we overwrite it. We need its audio/ipa.
        let original_element = match element_map.get(&(key.clone(), lang.clone())) {
            Some(el) => el.clone(),
            None => continue, // Should be impossible, but safer to handle
        };

        if let Some(referenced_element) = element_map.get(&(referenced_word.clone(), lang.clone()))
        {
            let mut new_element = referenced_element.clone();
            new_element.key = key.clone();
            new_element.word = key.clone();
            new_element.dereferenced_text = Some(dereferenced_text);

            // **THE FIX**: Preserve the original audio/ipa if the root element doesn't have it.
            if new_element.audio.is_empty() {
                new_element.audio = original_element.audio;
            }
            if new_element.ipa.is_none() {
                new_element.ipa = original_element.ipa;
            }

            element_map.insert((key, lang), new_element);
        }
    }

    element_map.into_values().collect()
}

fn count_whitespace(s: &str) -> usize {
    s.chars().filter(|c| c.is_whitespace()).count()
}

fn parse_dereference(text: &[HyperlinkedText]) -> Option<(String, String)> {
    let mut of_index = None;
    let mut referenced_word = None;

    // Find "of" and the link that follows it.
    'outer: for (i, item) in text.iter().enumerate() {
        let current_str = match item {
            HyperlinkedText::Plain(s) => s,
            HyperlinkedText::Link(s) => s,
        };

        if current_str.trim() == "of" {
            // Look ahead for a link. It's often separated by a space, so at `i + 2`.
            if let Some(next_item) = text.get(i + 1) {
                if let HyperlinkedText::Link(word) = next_item {
                    of_index = Some(i);
                    referenced_word = Some(word.clone());
                    break 'outer;
                }
            }
            if let Some(next_item) = text.get(i + 2) {
                if let HyperlinkedText::Link(word) = next_item {
                    of_index = Some(i);
                    referenced_word = Some(word.clone());
                    break 'outer;
                }
            }
        }
    }

    let of_index = of_index?;
    let referenced_word = referenced_word?;

    // --- Apply lenient safety checks from your LLM log ---
    let mut before_text_len = 0;
    let mut space_count_before = 0;
    for item in text.iter().take(of_index) {
        let s = match item {
            HyperlinkedText::Plain(s) | HyperlinkedText::Link(s) => s,
        };
        before_text_len += s.chars().count();
        space_count_before += count_whitespace(s);
    }

    if before_text_len > 100 || space_count_before > 12 {
        return None;
    }

    // Check text *after* the link.
    let link_pos = text
        .iter()
        .position(|item| match item {
            HyperlinkedText::Link(w) => w == &referenced_word,
            _ => false,
        })
        .unwrap_or(of_index); // Fallback to 'of' index

    let mut chars_after = 0;
    for item in text.iter().skip(link_pos + 1) {
        match item {
            HyperlinkedText::Plain(s) | HyperlinkedText::Link(s) => {
                chars_after += s.chars().count()
            }
        }
    }

    // Allow more room for phonetic transcriptions, as discovered was necessary for "далее".
    if chars_after > 30 {
        return None;
    }

    // Reconstruct the "before" text accurately up to the word "of"
    let final_before_text = text
        .iter()
        .take(of_index)
        .map(|item| match item {
            HyperlinkedText::Plain(s) | HyperlinkedText::Link(s) => s.as_str(),
        })
        .collect::<String>();

    Some((format!("{} of", final_before_text.trim()), referenced_word))
}

#[cfg(test)]
mod tests {
    use super::*;
    use libdictdefinition::{Definition, HyperlinkedText};

    #[test]
    fn test_parse_dereference_bemerkt() {
        let input = vec![
            HyperlinkedText::Plain("past".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Plain("participle".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Plain("of".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Link("bemerken".to_string()),
        ];
        let expected = Some(("past participle of".to_string(), "bemerken".to_string()));
        assert_eq!(parse_dereference(&input), expected);
    }

    #[test]
    fn test_parse_dereference_russian_with_phonetics() {
        // This is the "далее" -> "дальше" case that was failing.
        let input = vec![
            HyperlinkedText::Plain("Alternative".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Plain("form".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Plain("of".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Link("дальше".to_string()),
            HyperlinkedText::Plain(" (".to_string()),
            HyperlinkedText::Plain("dálʹše".to_string()),
            HyperlinkedText::Plain("): ".to_string()),
            HyperlinkedText::Plain("farther".to_string()),
        ];
        // With lenient checks, this should now pass.
        let expected = Some(("Alternative form of".to_string(), "дальше".to_string()));
        assert_eq!(parse_dereference(&input), expected);
    }

    #[test]
    fn test_parse_dereference_too_long_after() {
        // This test ensures the safety check still works.
        let input = vec![
            HyperlinkedText::Plain("form".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Plain("of".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Link("word".to_string()),
            HyperlinkedText::Plain(" and a very long sentence follows here that should definitely fail the thirty character safety check".to_string()),
        ];
        assert_eq!(parse_dereference(&input), None);
    }
}
