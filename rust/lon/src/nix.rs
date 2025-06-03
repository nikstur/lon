use std::{fmt, process::Command};

use anyhow::{Context, Result, bail};
use nix_compat::nixhash::NixHash;
use serde::{Deserialize, Serialize};

/// A SRI hash.
#[derive(Deserialize, Serialize, Clone)]
pub struct SriHash(NixHash);

impl SriHash {
    /// Convert a Nix Bas32 hash to a Sha256 SRI hash.
    fn from_nix_base_32(nix_base32_hash: &str) -> Result<Self> {
        let hash = nix_compat::nixhash::from_str(nix_base32_hash, Some("sha256"))?;
        Ok(Self(hash))
    }
}

impl fmt::Display for SriHash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.to_sri_string())
    }
}

#[derive(Deserialize)]
struct NixPrefetchGitResponse {
    sha256: String,
}

/// Fetch a git source and calculate its hash.
///
/// Uses the same store path (via `--name source`) as `builtins.fetchGit` to download the
/// source only once.
pub fn prefetch_git(url: &str, revision: &str, submodules: bool) -> Result<SriHash> {
    let mut command = Command::new("nix-prefetch-git");
    if submodules {
        command.arg("--fetch-submodules");
    }
    let output = command
        .arg("--name")
        .arg("source")
        .arg(url)
        .arg(revision)
        .output()
        .context("Failed to execute nix-prefetch-git. Most likely it's not on PATH")?;

    if !output.status.success() {
        bail!(
            "Failed to prefetch git from {url}@{revision}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let response: NixPrefetchGitResponse = serde_json::from_slice(&output.stdout)
        .context("Failed to deserialize nix-prefetch-git JSON response")?;

    SriHash::from_nix_base_32(&response.sha256)
}

/// Fetch a tarball and calculate its hash.
///
/// Uses the same store path (via `--name source`) as `builtins.fetchTarball` to download the
/// source only once.
pub fn prefetch_tarball(url: &str) -> Result<SriHash> {
    let output = Command::new("nix-prefetch-url")
        .arg("--unpack")
        .arg("--name")
        .arg("source")
        .arg("--type")
        .arg("sha256")
        .arg(url)
        .output()
        .context("Failed to execute nix-prefetch-url. Most likely it's not on PATH")?;

    if !output.status.success() {
        bail!(
            "Failed to prefetch tarball from {url}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    SriHash::from_nix_base_32(stdout.trim())
}
