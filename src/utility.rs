use std::time::{SystemTime, UNIX_EPOCH};



pub fn get_millis_timestamp(dt: std::time::SystemTime) -> Result<u64, Box<dyn std::error::Error>> {
    Ok(dt.duration_since(UNIX_EPOCH)?.as_millis() as u64)
}

pub fn get_current_timestamp() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    return since_the_epoch.as_millis() as u64;
}
