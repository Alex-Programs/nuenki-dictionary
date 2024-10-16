use std::fs::File;
use std::io::Write;
use std::path::Path;

use libdictdefinition::CompressedDictionaryElementWrapper;

pub fn output_compressed_dict(
    compressed_data: &[CompressedDictionaryElementWrapper],
    output_path: &Path,
) -> std::io::Result<()> {
    let encoded: Vec<u8> = bincode::serialize(&compressed_data).unwrap();
    let mut file = File::create(output_path)?;
    file.write_all(&encoded)?;
    Ok(())
}
