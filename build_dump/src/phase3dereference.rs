use libdictdefinition::{DictionaryElementData, HyperlinkedText};
use std::collections::HashMap;
use Languages::TargetLanguage;

pub fn process_dereferences(elements: Vec<DictionaryElementData>) -> Vec<DictionaryElementData> {
    let mut element_map: HashMap<(String, TargetLanguage), DictionaryElementData> = elements
        .into_iter()
        .map(|e| ((e.key.clone(), e.lang.clone()), e))
        .collect();

    let mut to_process = Vec::new();

    // Identify elements to be dereferenced
    for ((key, lang), element) in &element_map {
        if element.definitions.len() <= 3 {
            if let Some(first_def) = element.definitions.first() {
                if !first_def.tags.contains(&"Form-of".to_string()) {
                    continue;
                }

                if let Some((dereferenced_text, referenced_word)) =
                    parse_dereference(&first_def.text)
                {
                    to_process.push((
                        key.clone(),
                        lang.clone(),
                        dereferenced_text,
                        referenced_word,
                    ));

                    println!("Planning to process {} {:?}", key, lang);
                }
            }
        }
    }

    // Perform dereferencing
    let mut i = 0;
    let tpl = to_process.len() as f32;

    for (key, lang, dereferenced_text, referenced_word) in to_process {
        i += 1;

        if let Some(referenced_element) = element_map.get(&(referenced_word.clone(), lang.clone()))
        {
            let mut new_element = referenced_element.clone();
            new_element.key = key.clone();

            new_element.dereferenced_text = Some(dereferenced_text);
            element_map.insert((key, lang), new_element);
        }

        if i % 10000 == 0 {
            let percentage = i as f32 / tpl * 100.0;
            println!("{}%", percentage);
        }
    }

    element_map.into_values().collect()
}

fn count_whitespace(s: &str) -> usize {
    s.chars().filter(|c| c.is_whitespace()).count()
}

fn parse_dereference(text: &[HyperlinkedText]) -> Option<(String, String)> {
    let mut of_index = None;
    let mut char_count_before_of = 0;
    let mut space_count_before_of = 0;
    let mut referenced_word = None;

    let mut before_text = String::new();

    for (i, item) in text.iter().enumerate() {
        //println!("{:?}", item);
        match item {
            HyperlinkedText::Plain(s) | HyperlinkedText::Link(s) => {
                if s == "of" && i + 2 < text.len() {
                    if let HyperlinkedText::Link(word) = &text[i + 2] {
                        of_index = Some(i);
                        referenced_word = Some(word.clone());
                        break;
                    }
                }
                char_count_before_of += s.len();
                space_count_before_of += count_whitespace(s);

                before_text += s;
            }
        }

        if char_count_before_of > 50 || space_count_before_of >= 5 {
            return None;
        }
    }

    //println!("{:?}", of_index);

    let of_index = of_index?;
    let referenced_word = referenced_word?;

    let mut chars_after = 0;
    let mut lb_after = false;
    let mut rb_after = true;

    for (i, item) in text.iter().enumerate() {
        if i <= of_index + 2 {
            continue;
        }

        match item {
            HyperlinkedText::Plain(s) | HyperlinkedText::Link(s) => {
                chars_after += s.len();
                //println!("after {}", s);

                if s.contains("(") {
                    lb_after = true;
                }
                if s.contains(")") {
                    rb_after = true;
                }
            }
        }

        if chars_after > 15 {
            return None;
        }
    }

    if chars_after > 3 && (lb_after == false || rb_after == false) {
        return None;
    }

    Some((before_text + "of", referenced_word))
}

#[cfg(test)]
mod tests {
    use super::*;
    use libdictdefinition::HyperlinkedText;

    #[test]
    fn test_parse_dereference_versuchen() {
        let input = vec![
            HyperlinkedText::Plain("gerund".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Plain("of".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Link("versuchen".to_string()),
        ];

        let expected = Some(("gerund of".to_string(), "versuchen".to_string()));
        assert_eq!(parse_dereference(&input), expected);
    }
}
