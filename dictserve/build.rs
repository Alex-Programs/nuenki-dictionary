use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=lemmatization-cs.txt");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("czech_lemmas.rs");
    let mut out_file = File::create(&dest_path).unwrap();

    let file = File::open("lemmatization-cs.txt").unwrap();
    let reader = BufReader::new(file);

    writeln!(
        out_file,
        "use phf::phf_map; static CZECH_LEMMAS: phf::Map<&'static str, &'static str> = phf::phf_map! {{"
    )
    .unwrap();

    let mut seen = HashSet::new();

    for line in reader.lines() {
        let line = line.unwrap();
        // Skip empty lines and BOM
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() {
            continue;
        }

        if let Some((lemma, inflected)) = line.split_once('\t') {
            if seen.insert(inflected.to_string()) {
                writeln!(
                    out_file,
                    r#"    "{}" => "{}","#,
                    inflected.replace('"', r#"\""#),
                    lemma.replace('"', r#"\""#)
                )
                .unwrap();
            }
        }
    }

    writeln!(out_file, "}};").unwrap();
}
