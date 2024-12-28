use serde::{Deserialize, Serialize};
use Languages::TargetLanguage;

#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct Definition {
    pub text: Vec<HyperlinkedText>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum HyperlinkedText {
    Plain(String),
    Link(String),
}

// In libdictdefinition.rs
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DictionaryElementData {
    pub key: String,
    pub word: String,
    pub lang: TargetLanguage,
    pub audio: Vec<String>,
    pub ipa: Option<String>,
    pub word_types: Vec<String>,
    pub definitions: Vec<Definition>,
    pub dereferenced_text: Option<String>, // Add this line
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompressedDictionaryElementWrapper {
    pub key: String,
    pub lang: TargetLanguage,
    pub compressed_data: Vec<u8>,
}

impl DictionaryElementData {
    pub fn get_wiktionary_link(&self) -> String {
        let encoded_word = self.word.replace(" ", "");
        format!(
            "https://en.wiktionary.org/wiki/{}#{}",
            encoded_word,
            self.lang.to_wiktionary_long_name_n()
        )
    }
}
