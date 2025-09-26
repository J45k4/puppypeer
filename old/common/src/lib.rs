mod filehash;
mod files;
mod time;
pub mod metadata;

pub use filehash::calc_file_hash;
pub use filehash::calc_file_hash_async;
pub use filehash::convert_hash_to_string;
pub use files::provide_file_handle;
pub use time::get_current_timestamp;
pub use time::get_millis_timestamp;