pub fn get_version() -> u32 {
	match option_env!("VERSION") {
		Some(ver) => ver.parse().unwrap_or(0),
		None => 0,
	}
}

pub fn get_version_label() -> Option<&'static str> {
	option_env!("VERSION")
}
