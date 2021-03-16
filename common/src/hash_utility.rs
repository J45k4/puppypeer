use ring::digest::{Context, SHA256};
use std::io::Read;

pub fn calc_file_hash(file_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let file_path_string = file_path.to_string();
    
    let file = std::fs::File::open(file_path_string).unwrap();

    let mut reader = std::io::BufReader::new(file);

    let mut context = Context::new(&SHA256);
    let mut buffer = [0; 500000];

    loop {
        let count = reader.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }

        context.update(&buffer[..count]);
    }

    let digest = context.finish();

    let hash = digest.as_ref().iter().fold(String::with_capacity(
        digest.as_ref().len()), 
        |a, c| a + &format!("{:x?}", c) );

    Ok(hash)
}