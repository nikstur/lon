use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::nix::SriHash;

#[derive(Deserialize, Serialize)]
pub struct Lock {
    pub sources: BTreeMap<String, Source>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Source {
    Git(GitSource),
    GitHub(GitHubSource),
}

/// This type indicates what fetcher to use to download this source.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FetchType {
    Git,
    Tarball,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSource {
    pub fetch_type: FetchType,

    pub branch: String,
    pub revision: String,
    pub url: String,
    pub hash: SriHash,
    /// Whether to fetch submodules
    #[serde(default)]
    pub submodules: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubSource {
    pub fetch_type: FetchType,

    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub revision: String,
    pub url: String,
    pub hash: SriHash,
}
