use std::{
    collections::{BTreeMap, btree_map::Iter},
    fmt,
    path::Path,
};

use anyhow::{Context, Result, bail};
use regex_lite::Regex;
use reqwest::{
    Url,
    blocking::Client,
    header::{LINK, LOCATION},
    redirect::Policy,
};
use serde::Deserialize;

use crate::{
    git::{self, RevList, Revision},
    http::GitHubRepoApi,
    lock,
    nix::{self, SriHash},
};

const GITHUB_URL: &str = "https://github.com";

/// Informaton summarizing an update.
///
/// Represents an update of a single source.
#[derive(Clone)]
pub enum UpdateSummary {
    Rev(RevisionUpdate),
    Url(UrlUpdate),
}

impl UpdateSummary {
    pub fn from_revs(old_revision: Revision, new_revision: Revision) -> Self {
        Self::Rev(RevisionUpdate::new(old_revision, new_revision))
    }

    pub fn from_urls(old_url: &str, new_url: &str) -> Self {
        Self::Url(UrlUpdate::new(old_url, new_url))
    }
}

/// Represents an update of a versioned source.
#[derive(Clone)]
pub struct RevisionUpdate {
    pub old_revision: Revision,
    pub new_revision: Revision,
    pub rev_list: Option<RevList>,
}

/// Represents an update of a url source
#[derive(Clone)]
pub struct UrlUpdate {
    pub old_url: String,
    pub new_url: String,
}

impl RevisionUpdate {
    /// Create a new revision update summary.
    ///
    /// Tries to determine the revision
    pub fn new(old_revision: Revision, new_revision: Revision) -> Self {
        Self {
            old_revision,
            new_revision,
            rev_list: None,
        }
    }

    pub fn add_rev_list(&mut self, rev_list: RevList) {
        self.rev_list = Some(rev_list);
    }
}

impl UrlUpdate {
    /// Create a new revision update summary.
    pub fn new(old_url: &str, new_url: &str) -> Self {
        Self {
            old_url: old_url.to_string(),
            new_url: new_url.to_string(),
        }
    }
}

#[derive(Default, Clone)]
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
    pub fn write(&self, directory: impl AsRef<Path>) -> Result<()> {
        let lock = self.clone().into_latest_lock();
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

    /// Return an iterator over the sources
    pub fn iter(&self) -> Iter<'_, String, Source> {
        self.map.iter()
    }
}

#[derive(Clone)]
pub enum Source {
    Git(GitSource),
    GitHub(GitHubSource),
    Tarball(TarballSource),
}

impl Source {
    pub fn update(&mut self) -> Result<Option<UpdateSummary>> {
        match self {
            Self::Git(s) => s.update(),
            Self::GitHub(s) => s.update(),
            Self::Tarball(s) => s.update(),
        }
    }

    pub fn modify(
        &mut self,
        branch: Option<&String>,
        revision: Option<&String>,
        url: Option<&String>,
        hash: bool,
    ) -> Result<()> {
        match self {
            Self::Git(s) => s.modify(branch, revision, url, hash),
            Self::GitHub(s) => s.modify(branch, revision, url, hash),
            Self::Tarball(s) => s.modify(branch, revision, url, hash),
        }
    }

    pub fn freeze(&mut self) {
        match self {
            Self::Git(s) => s.frozen = true,
            Self::GitHub(s) => s.frozen = true,
            Self::Tarball(s) => s.frozen = true,
        }
    }

    pub fn unfreeze(&mut self) {
        match self {
            Self::Git(s) => s.frozen = false,
            Self::GitHub(s) => s.frozen = false,
            Self::Tarball(s) => s.frozen = false,
        }
    }

    // Return whether source is frozen.
    pub fn frozen(&self) -> bool {
        match self {
            Self::Git(s) => s.frozen,
            Self::GitHub(s) => s.frozen,
            Self::Tarball(s) => s.frozen,
        }
    }

    pub fn rev_list(&self, update: &RevisionUpdate, num_commits: usize) -> Result<Option<RevList>> {
        match self {
            Self::Git(s) => git::rev_list(
                &s.url,
                update.old_revision.as_str(),
                update.new_revision.as_str(),
                num_commits,
            ),
            Self::GitHub(s) => {
                let github_repo_api =
                    GitHubRepoApi::builder(&format!("{}/{}", s.owner, s.repo)).build()?;

                github_repo_api.compare_commits(
                    update.old_revision.as_str(),
                    update.new_revision.as_str(),
                    num_commits,
                )
            }
            Self::Tarball(_) => Ok(None),
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Source::Git(s) => {
                write!(f, "  Type: Git\n{s}")
            }
            Source::GitHub(s) => {
                write!(f, "  Type: GitHub\n{s}")
            }
            Source::Tarball(s) => {
                write!(f, "  Type: Tarball\n{s}")
            }
        }
    }
}

#[derive(Clone)]
pub struct GitSource {
    url: String,
    branch: String,
    revision: Revision,
    hash: SriHash,
    last_modified: Option<u64>,

    /// Whether to fetch submodules
    submodules: bool,

    frozen: bool,
}

impl fmt::Display for GitSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        [
            ("Url", &self.url),
            ("Branch", &self.branch),
            ("Revision", &self.revision.as_str().into()),
            ("Frozen", &self.frozen.to_string()),
        ]
        .iter()
        .try_for_each(|(k, v)| writeln!(f, "  {k}: {v}"))
    }
}

impl GitSource {
    pub fn new(
        url: &str,
        branch: Option<&String>,
        revision: Option<&String>,
        submodules: bool,
        frozen: bool,
    ) -> Result<Self> {
        let branch = match branch {
            Some(branch) => branch,
            None => &git::find_default_branch(url)?,
        };

        let rev = match revision {
            Some(rev) => rev,
            None => &git::find_newest_revision(url, branch)?.to_string(),
        };
        log::info!("Locked revision: {rev}");

        let hash = Self::compute_hash(url, rev, submodules)?;
        log::info!("Locked hash: {hash}");

        let last_modified = git::get_last_modified(url, rev)?;
        log::info!("Locked lastModified: {last_modified}");

        Ok(Self {
            url: url.into(),
            branch: branch.into(),
            revision: Revision::new(rev),
            hash,
            last_modified: Some(last_modified),
            submodules,
            frozen,
        })
    }

    /// Update the source by finding the newest commit.
    fn update(&mut self) -> Result<Option<UpdateSummary>> {
        if self.frozen {
            log::info!("Source is frozen");
            return Ok(None);
        }

        let newest_revision = git::find_newest_revision(&self.url, &self.branch)?;

        let current_revision = self.revision.clone();

        if current_revision == newest_revision {
            log::info!("Already up to date");
            return Ok(None);
        }
        log::info!("Updated revision: {current_revision} → {newest_revision}");
        self.lock(&newest_revision)?;
        Ok(Some(UpdateSummary::from_revs(
            current_revision,
            newest_revision,
        )))
    }

    /// Lock the source to a new revision.
    ///
    /// In this case this means that the revision and hash.
    fn lock(&mut self, revision: &Revision) -> Result<()> {
        let new_hash = Self::compute_hash(&self.url, revision.as_str(), self.submodules)?;
        log::info!("Updated hash: {} → {}", self.hash, new_hash);
        self.revision = revision.clone();
        self.hash = new_hash;
        let last_modified = git::get_last_modified(self.url.as_str(), revision.as_str())?;
        if let Some(value) = self.last_modified {
            log::info!("Updated lastModified: {value} → {last_modified}");
        } else {
            log::info!("Added lastModified: {last_modified}");
        }
        self.last_modified = Some(last_modified);
        Ok(())
    }

    /// Modify the source by changing its branch and/or its revision.
    fn modify(
        &mut self,
        branch: Option<&String>,
        revision: Option<&String>,
        url: Option<&String>,
        hash: bool,
    ) -> Result<()> {
        let mut rehash = hash;
        if let Some(branch) = branch {
            if self.branch == *branch {
                log::info!("Branch is already {branch}");
            } else {
                log::info!("Changed branch: {} → {}", self.branch, branch);
                self.branch = branch.into();
                if revision.is_none() {
                    if !self.update()?.is_none() {
                        // skip rehash if the update did something
                        rehash = false;
                    };
                }
            }
        }
        if let Some(revision) = revision {
            if self.revision.as_str() == revision {
                log::info!("Revision is already {revision}");
            } else {
                log::info!("Changed revision: {} → {}", self.revision, revision);
                self.lock(&Revision::new(revision))?;
                rehash = false;
            }
        }
        if url.is_some() {
            log::warn!("Cannot update URL of git sources");
        }
        if rehash {
            self.lock(&self.revision.clone())?;
        }
        Ok(())
    }

    /// Computing the hash for this source type.
    fn compute_hash(url: &str, revision: &str, submodules: bool) -> Result<SriHash> {
        nix::prefetch_git(url, revision, submodules)
            .with_context(|| format!("Failed to compute hash for {url}@{revision}"))
    }
}

#[derive(Clone)]
pub struct GitHubSource {
    owner: String,
    repo: String,
    branch: String,
    revision: Revision,
    url: String,
    hash: SriHash,

    frozen: bool,
}

impl fmt::Display for GitHubSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        [
            ("Repository", &format!("{}/{}", self.owner, self.repo)),
            ("Branch", &self.branch),
            ("Revision", &self.revision.as_str().into()),
            ("Frozen", &self.frozen.to_string()),
        ]
        .iter()
        .try_for_each(|(k, v)| writeln!(f, "  {k}: {v}"))
    }
}

impl GitHubSource {
    pub fn new(
        owner: &str,
        repo: &str,
        branch: Option<&String>,
        revision: Option<&String>,
        frozen: bool,
    ) -> Result<Self> {
        let repo_url = &Self::git_url(owner, repo);

        let branch = match branch {
            Some(branch) => branch,
            None => &git::find_default_branch(repo_url)?,
        };

        let rev = match revision {
            Some(rev) => rev,
            None => &git::find_newest_revision(repo_url, branch)?.to_string(),
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
            frozen,
        })
    }

    /// Update the source by finding the newest commit.
    fn update(&mut self) -> Result<Option<UpdateSummary>> {
        if self.frozen {
            log::info!("Source is frozen");
            return Ok(None);
        }

        let newest_revision =
            git::find_newest_revision(&Self::git_url(&self.owner, &self.repo), &self.branch)?;

        let current_revision = self.revision.clone();

        if current_revision == newest_revision {
            log::info!("Already up to date");
            return Ok(None);
        }

        log::info!("Updated revision: {current_revision} → {newest_revision}");
        self.lock(&newest_revision)?;
        Ok(Some(UpdateSummary::from_revs(
            current_revision,
            newest_revision,
        )))
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
    fn modify(
        &mut self,
        branch: Option<&String>,
        revision: Option<&String>,
        url: Option<&String>,
        hash: bool,
    ) -> Result<()> {
        let mut rehash = hash;
        if let Some(branch) = branch {
            if self.branch == *branch {
                log::info!("Branch is already {branch}");
            } else {
                log::info!("Changed branch: {} → {}", self.branch, branch);
                self.branch = branch.into();
                if revision.is_none() {
                    if !self.update()?.is_none() {
                        // skip rehash if the update did something
                        rehash = false;
                    };
                }
            }
        }
        if let Some(revision) = revision {
            if self.revision.as_str() == revision {
                log::info!("Revision is already {revision}");
            } else {
                log::info!("Changed revision: {} → {}", self.revision, revision);
                self.lock(&Revision::new(revision))?;
                rehash = false;
            }
        }
        if url.is_some() {
            log::warn!("Cannot update URL of GitHub sources");
        }
        if rehash {
            self.lock(&self.revision.clone())?;
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

#[derive(Clone)]
pub struct TarballSource {
    /// The URL that points to the latest version.
    ///
    /// The `url` field is resolved from the LINK tag returned by URL from the origin field.
    origin: Option<String>,
    url: String,
    hash: SriHash,
    revision: Option<String>,

    frozen: bool,
}

impl fmt::Display for TarballSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(origin) = &self.origin {
            writeln!(f, "  Origin: {origin}")?;
        }
        [("Locked", &self.url), ("Frozen", &self.frozen.to_string())]
            .iter()
            .try_for_each(|(k, v)| writeln!(f, "  {k}: {v}"))
    }
}

#[derive(Clone)]
struct TarballFlakeRef {
    url: String,

    tarball_ref: TarballRef,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TarballRef {
    nar_hash: Option<SriHash>,
    rev: Option<String>,
}

impl TarballSource {
    pub fn new(url: &str, frozen: bool) -> Result<Self> {
        let flake_ref_result = Self::resolve_flakeref_origin(url);
        if let Err(ref e) = flake_ref_result {
            log::debug!("{e}");
        }
        let source = if let Ok(flake_ref) = flake_ref_result {
            log::info!("Locked immutable URL: {}", flake_ref.url);
            if let Some(ref rev) = flake_ref.tarball_ref.rev {
                log::info!("Locked revision: {rev}");
            }
            let hash = Self::compute_hash(&flake_ref.url)?;
            // Check that the promised hash is the one we get
            if let Some(ref expected_hash) = flake_ref.tarball_ref.nar_hash
                && *expected_hash != hash
            {
                bail!("Hash mismatch: expected {expected_hash} but found {hash}");
            }
            log::info!("Locked hash: {hash}");

            Self {
                origin: Some(url.to_string()),
                revision: flake_ref.tarball_ref.rev,
                url: flake_ref.url,
                hash,
                frozen,
            }
        } else {
            log::info!("Source doesn't implement the Lockable HTTP Tarball Protocol");
            log::info!("Locked original URL: {url}");
            let hash = Self::compute_hash(url)?;
            log::info!("Locked hash: {hash}");

            Self {
                origin: None,
                revision: None,
                url: url.to_string(),
                hash,
                frozen,
            }
        };

        Ok(source)
    }

    pub fn from_channel(channel: &String, url: String, frozen: bool) -> Result<Self> {
        let locked_url = Url::parse(&url)?;
        let Some(mut path) = locked_url.path_segments() else {
            bail!("Invalid locked url: '{url}'");
        };

        let Some(target) = path.next_back() else {
            bail!("Missing target in url: '{url}'");
        };

        let hash = Self::compute_hash(&url)?;

        Ok(Self {
            origin: Some(format!("https://channels.nixos.org/{channel}/{target}")),
            revision: None,
            url,
            hash,
            frozen,
        })
    }

    /// Update the source.
    fn update(&mut self) -> Result<Option<UpdateSummary>> {
        if self.frozen {
            log::info!("Source is frozen");
            return Ok(None);
        }

        let Some(ref origin) = self.origin else {
            log::info!("Source is not updateable");
            return Ok(None);
        };

        // Recover the flakeref information
        let flake_ref = Self::resolve_flakeref_origin(origin)?;

        if self.url == flake_ref.url {
            log::info!("Already up to date");
            return Ok(None);
        }

        let current_url = self.url.clone();
        let newest_url = &flake_ref.url;
        log::info!("Updated URL: {current_url} → {newest_url}");

        // If the tarball has a revision associated to it use that in the update summary.
        let summary = if let (Some(rev), Some(new_rev)) =
            (self.revision.clone(), flake_ref.tarball_ref.rev.clone())
        {
            log::info!("Updated revision: {rev} → {new_rev}");
            self.revision = Some(new_rev.clone());

            UpdateSummary::from_revs(Revision::new(&rev), Revision::new(&new_rev))
        } else {
            if let Some(new_rev) = flake_ref.tarball_ref.rev {
                log::info!("Locked revision: {new_rev}");
                self.revision = Some(new_rev.clone());
            }

            UpdateSummary::from_urls(&current_url, newest_url)
        };

        self.lock(&flake_ref.url)?;

        Ok(Some(summary))
    }

    /// Lock the source to a specific URL.
    ///
    /// In this case this means that the hash, and URL is updated.
    fn lock(&mut self, url: &str) -> Result<()> {
        let new_hash = Self::compute_hash(url)?;
        log::info!("Updated hash: {} → {}", self.hash, new_hash);
        self.hash = new_hash;
        self.url = url.to_string();
        Ok(())
    }

    /// Modify the source by changing its branch and/or its revision.
    fn modify(
        &mut self,
        branch: Option<&String>,
        revision: Option<&String>,
        url: Option<&String>,
        hash: bool,
    ) -> Result<()> {
        let mut rehash = hash;
        if branch.is_some() {
            log::warn!("Cannot update branch of tarball sources");
        }
        if revision.is_some() {
            log::warn!("Cannot update revision of tarball source");
        }
        if let Some(url) = url {
            if self.origin.is_some() {
                log::warn!("Cannot update URL of this source because it's lockable");
            } else {
                if self.url == *url {
                    log::info!("URL is already {url}");
                } else {
                    log::info!("Changed URL: {} → {}", self.url, url);
                    self.lock(url)?;
                    rehash = false;
                }
            }
        }
        if rehash {
            self.lock(&self.url.clone())?;
        }
        Ok(())
    }

    /// Compute the hash for this source type.
    fn compute_hash(url: &str) -> Result<SriHash> {
        nix::prefetch_tarball(url).with_context(|| format!("Failed to compute hash for {url}"))
    }

    /// Return the target url of the tarball, it is either the locked url, according to the
    /// Lockable HTTP Tarball Protocol, or the origin url
    fn resolve_flakeref_origin(origin: &str) -> Result<TarballFlakeRef> {
        let client = Client::builder()
            .user_agent("Lon")
            // Do not follow redirect responses, as it will lose the Link header
            .redirect(Policy::none())
            .build()
            .context("Failed to build the HTTP client")?;

        let mut target = origin;
        let mut res;

        let re = Regex::new(r#"^<(?<flakeref>.*)>; rel="immutable"$"#)?;

        loop {
            res = client
                .head(target)
                .send()
                .context(format!("Failed to send HEAD request to {origin}"))?;

            if let Some(link) = res.headers().get(LINK) {
                // Parse the Link header value
                let link = link.to_str().expect("Failed to read the Link header value");

                if let Some(url) = re.captures(link).map(|m| m["flakeref"].to_string()) {
                    let data =
                        Url::parse(&url).context(format!("Failed to parse flakeref: {url}"))?;
                    let tarball_ref: TarballRef = serde_qs::from_str(data.query().unwrap_or(""))?;

                    return Ok(TarballFlakeRef { url, tarball_ref });
                }

                bail!("Failed to recover the flakeref of the tarball in the link: {link}")
            }

            if res.status().is_redirection() {
                let error = &format!(
                    "No location given in the redirection response from {}",
                    res.url()
                );

                target = res
                    .headers()
                    .get(LOCATION)
                    .expect(error)
                    .to_str()
                    .expect(error);
            } else {
                // We exhausted the redirection chain without finding a link header
                bail!(
                    "Link header not found, this source does not implement the Lockable HTTP Tarball Protocol"
                )
            }
        }
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

// V1 conversion
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
            lock::v1::Source::Tarball(s) => Self::Tarball(s.into()),
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
            last_modified: value.last_modified,
            submodules: value.submodules,
            frozen: value.frozen,
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
            frozen: value.frozen,
        }
    }
}

impl From<lock::v1::TarballSource> for TarballSource {
    fn from(value: lock::v1::TarballSource) -> Self {
        Self {
            frozen: value.frozen,
            origin: value.origin,
            revision: value.revision,
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
            Source::Tarball(s) => Self::Tarball(s.into()),
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
            last_modified: value.last_modified,
            submodules: value.submodules,
            frozen: value.frozen,
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
            frozen: value.frozen,
        }
    }
}

impl From<TarballSource> for lock::v1::TarballSource {
    fn from(value: TarballSource) -> Self {
        Self {
            fetch_type: lock::v1::FetchType::Tarball,
            frozen: value.frozen,
            origin: value.origin,
            revision: value.revision,
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
        let lock_json = include_str!("../tests/lon.lock");
        let lock = serde_json::from_str::<lock::v1::Lock>(lock_json)?;
        let sources = Sources::from(lock);
        let latest_lock = sources.into_latest_lock();
        let latest_lock_json = serde_json::to_string_pretty(&latest_lock)?;

        assert_eq!(lock_json, latest_lock_json);

        Ok(())
    }
}
