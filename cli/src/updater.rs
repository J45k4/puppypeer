use std::{
	io::BufReader,
	path::{Path, PathBuf},
};

use anyhow::bail;
use flate2::read::GzDecoder;
use rsa::signature::Verifier;
use rsa::{RsaPublicKey, pkcs1v15, pkcs8::DecodePublicKey};
use serde_json::Value;
use sha2::Sha256;
use tar::Archive;
use tokio::{fs::File, io::AsyncWriteExt};

use crate::utility::get_version;

// Path resolution: this file is cli/src/updater.rs; the key lives at repository root.
pub const PUBLIC_KEY: &str = include_str!("../../public_key.pem");

pub fn verify_signature(bin: &Path, sig: &Path) -> anyhow::Result<bool> {
	log::info!("verifying {} with {}", bin.display(), sig.display());
	let public_key = RsaPublicKey::from_public_key_pem(PUBLIC_KEY).unwrap();
	let verifying_key = pkcs1v15::VerifyingKey::<Sha256>::new(public_key);
	let signature = std::fs::read(sig)?;
	let signature = rsa::pkcs1v15::Signature::try_from(signature.as_slice())?;
	let data = std::fs::read(bin)?;
	let public_key = RsaPublicKey::from_public_key_pem(PUBLIC_KEY).unwrap();
	let verifying_key = pkcs1v15::VerifyingKey::<Sha256>::new(public_key);
	Ok(verifying_key.verify(&data, &signature).is_ok())
}

fn get_os_name() -> String {
	let os = std::env::consts::OS;
	let arch = std::env::consts::ARCH;
	format!("{}", os)
}

fn app_dir() -> PathBuf {
	let path = homedir::my_home().unwrap().unwrap().join(".puppypeer");
	if !path.exists() {
		std::fs::create_dir_all(&path).unwrap();
	}
	path
}

fn bin_dir() -> PathBuf {
	let path = app_dir().join("bin");
	if !path.exists() {
		std::fs::create_dir_all(&path).unwrap();
	}
	path
}

async fn fetch_release(version: Option<&str>) -> anyhow::Result<Value> {
	let client = reqwest::Client::new();
	let url = match version {
		Some(tag) => format!(
			"https://api.github.com/repos/j45k4/puppypeer/releases/tags/{}",
			tag
		),
		None => "https://api.github.com/repos/j45k4/puppypeer/releases/latest".to_string(),
	};
	let res = client
		.get(url)
		.header("User-Agent", "puppypeer")
		.send()
		.await?
		.error_for_status()?;
	let body = res.text().await?;

	Ok(serde_json::from_str::<Value>(&body)?)
}

async fn dowload_bin(url: &str, filename: &str) -> anyhow::Result<PathBuf> {
	let res = reqwest::get(url).await?;
	if !res.status().is_success() {
		bail!("Failed to download asset. HTTP status: {}", res.status());
	}
	let bytes = res.bytes().await?;
	let path = app_dir().join(&filename);
	let mut file = File::create(&path).await?;
	file.write_all(&bytes).await?;
	Ok(path)
}

pub async fn update(version: Option<&str>) -> anyhow::Result<()> {
	let res = fetch_release(version).await?;
	let tag = match res["tag_name"].as_str() {
		Some(tag) => tag,
		None => bail!("release response missing tag_name"),
	};
	let current = get_version();

	if let Some(requested_tag) = version {
		log::info!("requested tag: {}", requested_tag);
	}
	log::info!("current: {}", current);
	log::info!("release tag: {}", tag);

	if version.is_none() {
		if let Ok(tag_number) = tag.parse::<u32>() {
			log::info!("latest numeric tag: {}", tag_number);
			if tag_number <= current {
				log::info!("Already up to date");
				return Ok(());
			}
		} else {
			log::info!(
				"latest release tag {} is not numeric; skipping automatic version comparison",
				tag
			);
		}
	}

	let assets = match res["assets"] {
		Value::Array(ref assets) => assets,
		_ => bail!("no assets found"),
	};

	let os_name = get_os_name();
	let asset = match assets.iter().find(|asset| {
		if let Some(name) = asset["name"].as_str() {
			name.contains(&os_name)
		} else {
			false
		}
	}) {
		Some(asset) => asset,
		None => bail!("no asset found for os: {}", os_name),
	};

	let download_url = asset["browser_download_url"]
		.as_str()
		.ok_or_else(|| anyhow::anyhow!("no download url found"))?;

	log::info!("download_url: {}", download_url);

	// Attempt to derive a local filename from the asset name
	let filename = asset["name"]
		.as_str()
		.map(|s| s.to_string())
		.unwrap_or_else(|| "downloaded_binary".to_string());

	log::info!("Downloading asset: {}", filename);

	let path = dowload_bin(download_url, &filename).await?;

	log::info!("Downloaded asset to: {:?}", path);

	let file = std::fs::File::open(path)?;
	let buf_reader = BufReader::new(file);
	let decoder = GzDecoder::new(buf_reader);
	let mut archive = Archive::new(decoder);
	let mut entries = archive.entries()?;
	while let Some(file) = entries.next() {
		let mut file = file?;
		let name = match file.path() {
			Ok(name) => name,
			Err(_) => continue,
		};
		log::info!("unpacking: {:?}", name);
		let dst = app_dir().join(name);
		log::info!("unpacking to {:?}", dst);
		file.unpack(dst)?;
	}
	let bin_path = app_dir().join("puppypeer");
	let sig_path = bin_path.with_extension("sig");
	if !verify_signature(&bin_path, &sig_path)? {
		bail!("Signature verification failed");
	}
	tokio::fs::copy(&bin_path, bin_dir().join("puppypeer")).await?;
	tokio::fs::remove_file(&bin_path).await?;
	tokio::fs::remove_file(&sig_path).await?;
	Ok(())
}
