use std::{collections::BTreeMap, path::Path};

use anyhow::{Context, Result};

use crate::{
    git::{self, Revision},
    lock,
    nix::{self, SriHash},
};

const GITHUB_URL: &str = "https://github.com";

/// Informaton summarizing an update.
///
/// Represents an update of a single source.
pub struct UpdateSummary {
    pub old_revision: Revision,
    pub new_revision: Revision,
}

impl UpdateSummary {
    /// Create a new update summary.
    ///
    /// Tries to determine the revision
    pub fn new(old_revision: Revision, new_revision: Revision) -> Self {
        Self {
            old_revision,
            new_revision,
        }
    }
}

#[derive(Default)]
pub struct Sources {
    map: BTreeMap<String, Source>,
}

impl Sources {
    /// Read lock from a directory and convert to sources.
    pub fn read(directory: impl AsRef<Path>) -> Result<Self> {
        let lock = lock::Lock::read(directory)?;
        Ok(lock.into())
    }

    /// Convert to Lock and write to file inside the specified directory.
    pub fn write(self, directory: impl AsRef<Path>) -> Result<()> {
        let lock = self.into_latest_lock();
        lock.write(directory)?;
        Ok(())
    }

    /// Convert the sources to the latest lock format.
    pub fn into_latest_lock(self) -> lock::Lock {
        lock::Lock::V1(self.into())
    }

    /// Add a new source.
    pub fn add(&mut self, name: &str, source: Source) {
        self.map.insert(name.into(), source);
    }

    /// Remove a source.
    pub fn remove(&mut self, name: &str) {
        self.map.remove(name);
    }

    /// Get a mutable source.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Source> {
        self.map.get_mut(name)
    }

    /// Check whether a source is already inside the map
    pub fn contains(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    /// Return the list of source names.
    pub fn names(&self) -> Vec<&String> {
        self.map.keys().collect()
    }
}

pub enum Source {
    Git(GitSource),
    GitHub(GitHubSource),
}

impl Source {
    pub fn update(&mut self) -> Result<Option<UpdateSummary>> {
        match self {
            Self::Git(s) => s.update(),
            Self::GitHub(s) => s.update(),
        }
    }

    pub fn modify(&mut self, branch: Option<&String>, revision: Option<&String>) -> Result<()> {
        match self {
            Self::Git(s) => s.modify(branch, revision),
            Self::GitHub(s) => s.modify(branch, revision),
        }
    }
}

pub struct GitSource {
    url: String,
    branch: String,
    revision: Revision,
    hash: SriHash,

    /// Whether to fetch submodules
    submodules: bool,
}

impl GitSource {
    pub fn new(
        url: &str,
        branch: &str,
        revision: Option<&String>,
        submodules: bool,
    ) -> Result<Self> {
        let rev = match revision {
            Some(rev) => rev,
            None => &git::find_newest_revision(url, branch)?.to_string(),
        };
        log::info!("Locked revision: {rev}");

        let hash = Self::compute_hash(url, rev, submodules)?;
        log::info!("Locked hash: {hash}");

        Ok(Self {
            url: url.into(),
            branch: branch.into(),
            revision: Revision::new(rev),
            hash,
            submodules,
        })
    }

    /// Update the source by finding the newest commit.
    fn update(&mut self) -> Result<Option<UpdateSummary>> {
        let newest_revision = git::find_newest_revision(&self.url, &self.branch)?;

        let current_revision = self.revision.clone();

        if current_revision == newest_revision {
            log::info!("Already up to date");
            return Ok(None);
        };
        log::info!("Updated revision: {current_revision} → {newest_revision}");
        self.lock(&newest_revision)?;
        Ok(Some(UpdateSummary::new(current_revision, newest_revision)))
    }

    /// Lock the source to a new revision.
    ///
    /// In this case this means that the revision and hash.
    fn lock(&mut self, revision: &Revision) -> Result<()> {
        let new_hash = Self::compute_hash(&self.url, revision.as_str(), self.submodules)?;
        log::info!("Updated hash: {} → {}", self.hash, new_hash);
        self.revision = revision.clone();
        self.hash = new_hash;
        Ok(())
    }

    /// Modify the source by changing its branch and/or its revision.
    fn modify(&mut self, branch: Option<&String>, revision: Option<&String>) -> Result<()> {
        if let Some(branch) = branch {
            if self.branch == *branch {
                log::info!("Branch is already {branch}");
            } else {
                log::info!("Changed branch: {} → {}", self.branch, branch);
                self.branch = branch.into();
                if revision.is_none() {
                    self.update()?;
                };
            }
        }
        if let Some(revision) = revision {
            if self.revision.as_str() == revision {
                log::info!("Revision is already {revision}");
            } else {
                log::info!("Changed revision: {} → {}", self.revision, revision);
                self.lock(&Revision::new(revision))?;
            }
        }
        Ok(())
    }

    /// Computing the hash for this source type.
    fn compute_hash(url: &str, revision: &str, submodules: bool) -> Result<SriHash> {
        nix::prefetch_git(url, revision, submodules)
            .with_context(|| format!("Failed to compute hash for {url}@{revision}"))
    }
}

pub struct GitHubSource {
    owner: String,
    repo: String,
    branch: String,
    revision: Revision,
    url: String,
    hash: SriHash,
}

impl GitHubSource {
    pub fn new(owner: &str, repo: &str, branch: &str, revision: Option<&String>) -> Result<Self> {
        let rev = match revision {
            Some(rev) => rev,
            None => &git::find_newest_revision(&Self::git_url(owner, repo), branch)?.to_string(),
        };
        log::info!("Locked revision: {rev}");

        let url = Self::url(owner, repo, rev);

        let hash = Self::compute_hash(&url)?;
        log::info!("Locked hash: {hash}");

        Ok(Self {
            owner: owner.into(),
            repo: repo.into(),
            url,
            branch: branch.into(),
            revision: Revision::new(rev),
            hash,
        })
    }

    /// Update the source by finding the newest commit.
    fn update(&mut self) -> Result<Option<UpdateSummary>> {
        let newest_revision =
            git::find_newest_revision(&Self::git_url(&self.owner, &self.repo), &self.branch)?;

        let current_revision = self.revision.clone();

        if current_revision == newest_revision {
            log::info!("Already up to date");
            return Ok(None);
        };

        log::info!("Updated revision: {current_revision} → {newest_revision}");
        self.lock(&newest_revision)?;
        Ok(Some(UpdateSummary::new(current_revision, newest_revision)))
    }

    /// Lock the source to a specific revision.
    ///
    /// In this case this means that the revision, hash, and URL is updated.
    fn lock(&mut self, revision: &Revision) -> Result<()> {
        let new_url = Self::url(&self.owner, &self.repo, revision.as_str());
        let new_hash = Self::compute_hash(&new_url)?;
        log::info!("Updated hash: {} → {}", self.hash, new_hash);
        self.revision = revision.clone();
        self.hash = new_hash;
        self.url = new_url;
        Ok(())
    }

    /// Modify the source by changing its branch and/or its revision.
    fn modify(&mut self, branch: Option<&String>, revision: Option<&String>) -> Result<()> {
        if let Some(branch) = branch {
            if self.branch == *branch {
                log::info!("Branch is already {branch}");
            } else {
                log::info!("Changed branch: {} → {}", self.branch, branch);
                self.branch = branch.into();
                if revision.is_none() {
                    self.update()?;
                };
            }
        }
        if let Some(revision) = revision {
            if self.revision.as_str() == revision {
                log::info!("Revision is already {revision}");
            } else {
                log::info!("Changed revision: {} → {}", self.revision, revision);
                self.lock(&Revision::new(revision))?;
            }
        }
        Ok(())
    }

    /// Compute the hash for this source type.
    fn compute_hash(url: &str) -> Result<SriHash> {
        nix::prefetch_tarball(url).with_context(|| format!("Failed to compute hash for {url}"))
    }

    /// Return the URL to a GitHub tarball for the revision of the source.
    fn url(owner: &str, repo: &str, revision: &str) -> String {
        format!("{GITHUB_URL}/{owner}/{repo}/archive/{revision}.tar.gz")
    }

    /// Return the URL to the GitHub repository.
    fn git_url(owner: &str, repo: &str) -> String {
        format!("{GITHUB_URL}/{owner}/{repo}.git")
    }
}

// Boilerplate to convert between the internal representation (Sources) and the external lock file
// representation.
//
// This seems like a lot of duplication but it is mostly incidental duplication. Once we add more
// lockfile versions this'll become clear.

impl From<lock::Lock> for Sources {
    fn from(value: lock::Lock) -> Self {
        match value {
            lock::Lock::V1(l) => Sources::from(l),
        }
    }
}

impl From<lock::v1::Lock> for Sources {
    fn from(value: lock::v1::Lock) -> Self {
        let map = value
            .sources
            .into_iter()
            .map(|(k, s)| (k, s.into()))
            .collect::<BTreeMap<_, _>>();
        Self { map }
    }
}

impl From<lock::v1::Source> for Source {
    fn from(value: lock::v1::Source) -> Self {
        match value {
            lock::v1::Source::Git(s) => Self::Git(s.into()),
            lock::v1::Source::GitHub(s) => Self::GitHub(s.into()),
        }
    }
}

impl From<lock::v1::GitSource> for GitSource {
    fn from(value: lock::v1::GitSource) -> Self {
        Self {
            branch: value.branch,
            revision: Revision::new(&value.revision),
            url: value.url,
            hash: value.hash,
            submodules: value.submodules,
        }
    }
}

impl From<lock::v1::GitHubSource> for GitHubSource {
    fn from(value: lock::v1::GitHubSource) -> Self {
        Self {
            owner: value.owner,
            repo: value.repo,
            branch: value.branch,
            revision: Revision::new(&value.revision),
            url: value.url,
            hash: value.hash,
        }
    }
}

impl From<Sources> for lock::v1::Lock {
    fn from(value: Sources) -> Self {
        let sources = value
            .map
            .into_iter()
            .map(|(k, s)| (k, s.into()))
            .collect::<BTreeMap<_, _>>();
        Self { sources }
    }
}

impl From<Source> for lock::v1::Source {
    fn from(value: Source) -> Self {
        match value {
            Source::Git(s) => Self::Git(s.into()),
            Source::GitHub(s) => Self::GitHub(s.into()),
        }
    }
}

impl From<GitSource> for lock::v1::GitSource {
    fn from(value: GitSource) -> Self {
        Self {
            fetch_type: lock::v1::FetchType::Git,
            branch: value.branch,
            revision: value.revision.to_string(),
            url: value.url,
            hash: value.hash,
            submodules: value.submodules,
        }
    }
}

impl From<GitHubSource> for lock::v1::GitHubSource {
    fn from(value: GitHubSource) -> Self {
        Self {
            fetch_type: lock::v1::FetchType::Tarball,
            owner: value.owner,
            repo: value.repo,
            branch: value.branch,
            revision: value.revision.to_string(),
            url: value.url,
            hash: value.hash,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::Result;

    /// Parsing to internal representation and converting it back produces the same representation.
    #[test]
    fn parse_and_convert() -> Result<()> {
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
                }
            }
        });

        let lock = serde_json::from_value::<lock::v1::Lock>(value.clone())?;
        let sources = Sources::from(lock);
        let latest_lock = sources.into_latest_lock();
        let latest_value = serde_json::to_value(latest_lock)?;

        assert_eq!(value, latest_value);

        Ok(())
    }
}
