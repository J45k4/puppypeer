use ring::digest::{Context, SHA256};
use std::io::Read;

pub async fn calc_file_hash_async(file_path: &str) -> Result<Vec<u8>, failure::Error> {
    let file_path = file_path.to_string();
    let hash = tokio::task::spawn_blocking(move || {
        calc_file_hash(&file_path)
    }).await??;

    Ok(hash)
}

pub fn calc_file_hash(file_path: &str) -> Result<Vec<u8>, failure::Error> {
    let file_path_string = file_path.to_string();
    
    let file = std::fs::File::open(file_path_string).unwrap();

    let mut reader = std::io::BufReader::new(file);

    let mut context = Context::new(&SHA256);
    let mut buffer = [0; 1_000_000];

    loop {
        let count = reader.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }

        context.update(&buffer[..count]);
    }

    let digest = context.finish();

    Ok(digest.as_ref().to_vec())
}

pub fn convert_hash_to_string(hash: &[u8]) -> String {
    hash.iter().fold(String::with_capacity(
        hash.len()), 
        |a, c| a + &format!("{:x?}", c) )
}