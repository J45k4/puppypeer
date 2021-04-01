mod hash_utility;
mod files;
mod time;
pub mod metadata;

pub use hash_utility::calc_file_hash;
pub use files::provide_file_handle;
pub use time::get_current_timestamp;
pub use time::get_millis_timestamp;