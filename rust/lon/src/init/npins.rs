use std::{collections::BTreeMap, path::Path};

use anyhow::{Context, Result, bail};
use regex_lite::Regex;
use serde::Deserialize;

use crate::{
    init::Convertible,
    sources::{GitHubSource, GitSource, Source, Sources},
};

#[derive(Debug, Deserialize)]
pub struct LockFile {
    pins: BTreeMap<String, Pin>,
    version: u64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Repository {
    Git {
        /// URL to the Git repository
        url: String,
    },
    Forgejo {
        server: String,
        owner: String,
        repo: String,
    },
    GitHub {
        /// "owner/repo"
        owner: String,
        repo: String,
    },
    GitLab {
        /// usually "owner/repo" or "group/owner/repo" (without leading or trailing slashes)
        repo_path: String,
        /// Of the kind <https://gitlab.example.org/>
        ///
        /// It must fit into the schema `<server>/<owner>/<repo>` to get a repository's URL.
        server: String,
        /// access token for private repositories
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(default)]
        private_token: Option<String>,
    },
}

// HACK: We know that a Git pin has a branch associated to it and GitRelease has none,
//       but to unify the behaviour, we set them bot to `Option`s
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Pin {
    Git {
        repository: Repository,
        branch: Option<String>,
        revision: String,
        submodules: bool,
        #[serde(default)]
        frozen: bool,
    },
    GitRelease {
        repository: Repository,
        branch: Option<String>,
        revision: String,
        submodules: bool,
        #[serde(default)]
        frozen: bool,
    },
    Channel {
        #[serde(rename = "name")]
        channel: String,
        url: String,
        #[serde(default)]
        frozen: bool,
    },
}

impl LockFile {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let lock_json = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read {}", path.as_ref().display()))?;

        serde_json::from_str(&lock_json).context("Failed to deserialize Npins lock file")
    }
}

impl Convertible for LockFile {
    fn convert(&self) -> Result<Sources> {
        let mut sources = Sources::default();

        if self.version == 1 {
            bail!("Unsupported npins lockfile version: {}", self.version)
        }

        let re = Regex::new(
            r"https://releases\.nixos\.org/.*\.(?<shortrev>[a-f0-9]+)/nixexprs\.tar\.xz",
        )?;

        for (name, pin) in &self.pins {
            log::info!("Converting {name}...");

            let source = match pin {
                Pin::Channel {
                    channel,
                    url,
                    frozen,
                } => {
                    let Some(matched) = re.captures(url) else {
                        bail!("Cannot extract revision from the channel url: {url}")
                    };

                    Source::GitHub(GitHubSource::new(
                        "NixOS",
                        "nixpkgs",
                        Some(channel),
                        Some(&matched["shortrev"].into()),
                        *frozen,
                    )?)
                }
                Pin::Git {
                    repository,
                    branch,
                    revision,
                    submodules,
                    frozen,
                }
                | Pin::GitRelease {
                    repository,
                    branch,
                    revision,
                    submodules,
                    frozen,
                } => match repository {
                    Repository::Git { url } => Source::Git(GitSource::new(
                        url,
                        branch.as_ref(),
                        Some(revision),
                        *submodules,
                        *frozen,
                    )?),
                    Repository::GitHub { owner, repo } => {
                        if *submodules {
                            Source::Git(GitSource::new(
                                &format!("https://github.com/{owner}/{repo}"),
                                branch.as_ref(),
                                Some(revision),
                                *submodules,
                                *frozen,
                            )?)
                        } else {
                            Source::GitHub(GitHubSource::new(
                                owner,
                                repo,
                                branch.as_ref(),
                                Some(revision),
                                *frozen,
                            )?)
                        }
                    }
                    Repository::Forgejo {
                        server,
                        owner,
                        repo,
                    } => Source::Git(GitSource::new(
                        &format!("{server}/{owner}/{repo}"),
                        branch.as_ref(),
                        Some(revision),
                        *submodules,
                        *frozen,
                    )?),
                    Repository::GitLab {
                        repo_path,
                        server,
                        private_token,
                    } => {
                        if private_token.is_some() {
                            log::warn!(
                                "GitLab source {name} is configured with a PAT, which unsupported in lon"
                            );
                        }
                        Source::Git(GitSource::new(
                            &format!("{server}/{repo_path}"),
                            branch.as_ref(),
                            Some(revision),
                            *submodules,
                            *frozen,
                        )?)
                    }
                },
            };

            sources.add(name, source);
        }

        Ok(sources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl LockFile {
        fn from_str(s: &str) -> Result<Self> {
            serde_json::from_str(s).context("Failed to deserialize npins lock file")
        }
    }

    #[test]
    fn parse_npins_lock_file() -> Result<()> {
        LockFile::from_str(include_str!("../../tests/npins.json"))?;
        Ok(())
    }
}
