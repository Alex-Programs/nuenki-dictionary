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

    println!("{:?}", text);

    'outer: for (i, item) in text.iter().enumerate() {
        //println!("{:?}", item);
        match item {
            HyperlinkedText::Plain(s) | HyperlinkedText::Link(s) => {
                println!("|{}|", s);
                if *s == "of" {
                    for offset in 1..(2 + 1) {
                        if i + offset < text.len() {
                            if let HyperlinkedText::Link(word) = &text[i + offset] {
                                of_index = Some(i);
                                referenced_word = Some(word.clone());
                                break 'outer;
                            }
                        }
                    }
                }
                char_count_before_of += s.len();
                space_count_before_of += count_whitespace(s);

                before_text += s;
            }
        }

        if char_count_before_of > 70 || space_count_before_of >= 8 {
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

    Some((before_text + "of", referenced_word.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase2transform::hyperlink_text;
    use libdictdefinition::HyperlinkedText;

    #[test]
    fn test_sollen() {
        let input = vec![
            HyperlinkedText::Plain("inflection".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Plain("of".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Link("sollen".to_string()),
            HyperlinkedText::Plain(":".to_string()),
        ];

        let expected = Some(("inflection of".to_string(), "sollen".to_string()));
        assert_eq!(parse_dereference(&input), expected);
    }

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

    #[test]
    fn test_parse_dereference_latin() {
        let input = vec![
            HyperlinkedText::Plain("third".to_string()),
            HyperlinkedText::Plain("-".to_string()),
            HyperlinkedText::Plain("person".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Plain("singular".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Plain("present".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Link("active".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Link("indicative".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Plain("of".to_string()),
            HyperlinkedText::Link("operor".to_string()),
        ];

        let expected = Some((
            "third-person singular present active indicative of".to_string(),
            "operor".to_string(),
        ));
        assert_eq!(parse_dereference(&input), expected);
    }

    #[test]
    fn test_parse_dereference_no_of() {
        let input = vec![
            HyperlinkedText::Plain("example".to_string()),
            HyperlinkedText::Plain("text".to_string()),
        ];

        let expected = None;
        assert_eq!(parse_dereference(&input), expected);
    }

    #[test]
    fn test_parse_dereference_exceeds_limits() {
        let input = vec![
            HyperlinkedText::Plain("a".repeat(510)),
            HyperlinkedText::Plain("of".to_string()),
            HyperlinkedText::Link("word".to_string()),
        ];

        let expected = None;
        assert_eq!(parse_dereference(&input), expected);
    }

    #[test]
    fn test_parse_dereference_complex_but_valid() {
        let input = vec![
            HyperlinkedText::Plain("past participle".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Plain("of".to_string()),
            HyperlinkedText::Plain(" ".to_string()),
            HyperlinkedText::Link("be".to_string()),
        ];

        let expected = Some(("past participle of".to_string(), "be".to_string()));
        assert_eq!(parse_dereference(&input), expected);
    }

    #[test]
    fn test_count_whitespace_empty() {
        let input = "";
        let expected = 0;
        assert_eq!(count_whitespace(input), expected);
    }

    #[test]
    fn test_count_whitespace_spaces_and_tabs() {
        let input = "a b\tc  d";
        let expected = 4; // 3 spaces + 1 tab
        assert_eq!(count_whitespace(input), expected);
    }

    #[test]
    fn test_count_whitespace_no_whitespace() {
        let input = "abcdef";
        let expected = 0;
        assert_eq!(count_whitespace(input), expected);
    }

    #[test]
    fn test_process_dereferences_empty() {
        let input: Vec<DictionaryElementData> = vec![];
        let expected: Vec<DictionaryElementData> = vec![];
        assert_eq!(process_dereferences(input), expected);
    }
}
