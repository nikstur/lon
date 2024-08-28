use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use serde::{Deserialize, Serialize};

pub mod v1;

/// Lock containing all information necessary to retrieve the locked resources.
///
/// Only add a new version when it is backwards incompatible. Backwards compatible changes (e.g.
/// adding new fields) should be done in the same version.
#[derive(Deserialize, Serialize)]
#[serde(tag = "version")]
pub enum Lock {
    #[serde(rename = "1")]
    V1(v1::Lock),
}

impl Lock {
    const FILENAME: &'static str = "lon.lock";

    pub fn read(directory: impl AsRef<Path>) -> Result<Self> {
        Self::from_file(Self::path(directory))
    }

    pub fn write(&self, directory: impl AsRef<Path>) -> Result<()> {
        self.to_file(Self::path(directory))
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path.as_ref())
            .with_context(|| format!("Failed to open {:?}", path.as_ref()))?;
        serde_json::from_reader(file).context("Failed to deserialize lock file")
    }

    pub fn to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let mut file = File::create(path.as_ref())
            .with_context(|| format!("Failed to open {:?}", path.as_ref()))?;
        serde_json::to_writer_pretty(&mut file, self).context("Failed to serialize lock file")?;
        file.write_all(b"\n")?;
        Ok(())
    }

    pub fn path(directory: impl AsRef<Path>) -> PathBuf {
        directory.as_ref().join(Self::FILENAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::Result;

    #[test]
    fn parse_lock() -> Result<()> {
        let value = serde_json::json!({
            "version": "1",
            "sources": {
                "nixpkgs": {
                    "type": "GitHub",
                    "fetchType": "tarball",
                    "owner": "nixos",
                    "repo": "nixpkgs",
                    "revision": "a9858885e197f984d92d7fe64e9fff6b2e488d40",
                    "branch": "master",
                    "url": "https://github.com/nixos/nixpkgs/archive/a9858885e197f984d92d7fe64e9fff6b2e488d40.tar.gz",
                    "hash": "sha256-h1zQVhXuYoKTgJWqgVa7veoCJlbuG+xyzLQAar1Np5Y="
                },
                "lanzaboote": {
                    "type": "Git",
                    "fetchType": "git",
                    "branch": "master",
                    "revision": "f5a3a7dff44d131807fc1a89fbd8576cd870334a",
                    "url": "git@github.com:nix-community/lanzaboote.git",
                    "hash": "sha256-e/fSi0WER06N8WCvpht62fkGtWfe5ckDxr6zNYkwkFw=",
                },
            }
        });

        serde_json::from_value::<Lock>(value)?;
        Ok(())
    }
}
