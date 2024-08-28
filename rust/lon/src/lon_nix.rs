use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

pub struct LonNix;

impl LonNix {
    const FILENAME: &'static str = "lon.nix";

    const LON_NIX: &'static str = include_str!("lon.nix");
    const LON_NIX_SHA256: &'static [u8; 32] =
        include_bytes!(concat!(env!("OUT_DIR"), "lon.nix.sha256"));

    /// Update lon.nix.
    ///
    /// Only update if the file on disk doesn't match the hash of the currently embedded version.
    pub fn update(directory: impl AsRef<Path>) -> Result<()> {
        let actual_hash = hash_file(Self::path(&directory))
            .with_context(|| format!("Failed to hash {}", Self::FILENAME))?;

        if actual_hash != *Self::LON_NIX_SHA256 {
            log::info!("Updating lon.nix...");
            Self::write(directory)?;
        }
        Ok(())
    }

    /// Write lon.nix to disk.
    pub fn write(directory: impl AsRef<Path>) -> Result<()> {
        fs::write(Self::path(directory), Self::LON_NIX.as_bytes())
            .context("Failed to write lon.nix")
    }

    pub fn path(directory: impl AsRef<Path>) -> PathBuf {
        directory.as_ref().join(Self::FILENAME)
    }
}

/// Hash a file with SHA256.
fn hash_file(path: impl AsRef<Path>) -> Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    let mut file =
        File::open(&path).with_context(|| format!("Failed to open: {:?}", path.as_ref()))?;
    io::copy(&mut file, &mut hasher)?;

    let mut buffer = [0u8; 32];
    buffer.copy_from_slice(&hasher.finalize());

    Ok(buffer)
}
