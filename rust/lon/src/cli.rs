use std::{
    env,
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{
    bot::{Forge, Forgejo, GitHub, GitLab},
    commit_message::CommitMessage,
    git,
    init::{Convertible, niv, npins},
    lock::Lock,
    lon_nix::LonNix,
    sources::{GitHubSource, GitSource, Source, Sources, TarballSource, UpdateSummary},
};

/// The default log level.
///
/// 2 corresponds to the level INFO.
const DEFAULT_LOG_LEVEL: usize = 2;

#[derive(Parser)]
#[command(version)]
pub struct Cli {
    /// Silence all output
    #[arg(short, long)]
    quiet: bool,
    /// Verbose mode (-v, -vv, etc.)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    /// The directory containing lon.{nix,lock}
    #[arg(short, long)]
    directory: Option<PathBuf>,
    #[clap(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize lon.{nix,lock}
    Init(InitArgs),
    /// Add a new source
    Add {
        #[clap(subcommand)]
        commands: AddCommands,
    },
    /// Update an existing source to the newest revision
    Update(UpdateArgs),
    /// Modify an existing source
    ///
    /// When you only change the branch, the newest revision from that branch is locked.
    ///
    /// When you change the revision, the source is locked to this revision.
    Modify(ModifyArgs),
    /// Remove an existing source
    Remove(SourceArgs),
    /// Freeze an existing source
    Freeze(SourceArgs),
    /// Unfreeze an existing source
    Unfreeze(SourceArgs),
    /// Show the list of sources or a specific source
    Show { name: Option<String> },

    /// Bot that opens PRs for updates
    Bot {
        #[clap(subcommand)]
        commands: BotCommands,
    },
}

#[derive(Args)]
struct InitArgs {
    /// The type of lock file to initalize from
    #[arg(long, value_enum)]
    from: Option<LockFileType>,
    /// Path to the lock file to initialize from
    #[arg(long)]
    source: Option<PathBuf>,
}

#[derive(Clone, ValueEnum)]
enum LockFileType {
    Niv,
    Npins,
}

#[derive(Subcommand)]
#[clap(rename_all = "lower")]
enum AddCommands {
    /// Add a git source
    ///
    /// It's fetched by checking out the repository.
    Git(AddGitArgs),
    /// Add a github source
    ///
    /// It's fetched as a tarball which is more efficient than checking out the
    /// repository.
    GitHub(AddGitHubArgs),
    /// Add a github source
    ///
    /// It tries to use the immutable tarbal protocol if available
    Tarball(AddTarballArgs),
}

#[derive(Args)]
struct AddGitArgs {
    /// Name of the source
    name: String,
    /// URL to the repository
    url: String,
    /// Branch to track
    branch: Option<String>,
    /// Revision to lock
    #[arg(short, long)]
    revision: Option<String>,
    /// Fetch submodules
    #[arg(long)]
    submodules: bool,
    /// Freeze the source
    #[arg(long, default_value_t = false)]
    frozen: bool,
}

#[derive(Args)]
struct AddGitHubArgs {
    /// An identifier made up of {owner}/{repo}, e.g. nixos/nixpkgs
    identifier: String,
    /// Branch to track
    branch: Option<String>,
    /// Name of the source
    ///
    /// If you do not supply this, the repository name is used as the source name.
    #[arg(short, long)]
    name: Option<String>,
    /// Revision to lock
    #[arg(short, long)]
    revision: Option<String>,
    /// Freeze the source
    #[arg(long, default_value_t = false)]
    frozen: bool,
}

#[derive(Args)]
struct AddTarballArgs {
    /// Name of the source
    name: String,
    /// URL to the tarball
    url: String,
    /// Freeze the source
    #[arg(long, default_value_t = false)]
    frozen: bool,
}

#[derive(Args)]
struct UpdateArgs {
    /// Name of the source
    ///
    /// If this is omitted, all sources are updated.
    name: Option<String>,
    /// Whether to commit lon.{nix,lock}.
    #[arg(short, long, default_value_t = false)]
    commit: bool,
    /// Whether to continue to try to update sources
    /// when a source fails to update.
    #[arg(long, default_value_t = false)]
    r#continue: bool,
}

#[derive(Args)]
struct ModifyArgs {
    /// Name of the source
    name: String,
    /// Branch to track
    #[arg(short, long)]
    branch: Option<String>,
    /// Revision to lock
    #[arg(short, long)]
    revision: Option<String>,
    /// URL to change
    #[arg(short, long)]
    url: Option<String>,
}

#[derive(Args)]
struct SourceArgs {
    /// Name of the source
    name: String,
}

#[derive(Subcommand)]
#[clap(rename_all = "lower")]
enum BotCommands {
    /// Run the bot for GitLab
    GitLab,
    /// Run the bot for GitHub
    GitHub,
    /// Run the bot for Forgejo
    Forgejo,
}

impl Cli {
    pub fn init(module: &str) -> ExitCode {
        let cli = Self::parse();

        let _ = stderrlog::new()
            .module(module)
            .show_level(false)
            .quiet(cli.quiet)
            .verbosity(DEFAULT_LOG_LEVEL + usize::from(cli.verbose))
            .init();

        let directory = match cli.directory {
            Some(directory) => directory,
            None => match std::env::var("LON_DIRECTORY") {
                Ok(dir) => PathBuf::from(dir),
                Err(_) => std::env::current_dir().unwrap_or_default(),
            },
        };

        match cli.commands.call(directory) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                log::error!("{err:#}");
                ExitCode::FAILURE
            }
        }
    }
}

impl Commands {
    pub fn call(self, directory: impl AsRef<Path>) -> Result<()> {
        match self {
            Self::Init(args) => init(directory, &args),
            Self::Add { commands } => match commands {
                AddCommands::Git(args) => add_git(directory, &args),
                AddCommands::GitHub(args) => add_github(directory, &args),
                AddCommands::Tarball(args) => add_tarball(directory, &args),
            },
            Self::Update(args) => update(directory, &args),
            Self::Modify(args) => modify(directory, &args),
            Self::Remove(args) => remove(directory, &args),
            Self::Freeze(args) => freeze(directory, &args),
            Self::Unfreeze(args) => unfreeze(directory, &args),
            Self::Show { name } => show(directory, name),

            Self::Bot { commands } => match commands {
                BotCommands::GitLab => bot(directory, &GitLab::from_env()?),
                BotCommands::GitHub => bot(directory, &GitHub::from_env()?),
                BotCommands::Forgejo => bot(directory, &Forgejo::from_env()?),
            },
        }
    }
}

fn init(directory: impl AsRef<Path>, args: &InitArgs) -> Result<()> {
    if LonNix::path(&directory).exists() {
        log::info!("lon.nix already exists");
    } else {
        log::info!("Writing lon.nix...");
        LonNix::write(&directory)?;
    }

    if Lock::path(&directory).exists() {
        log::info!("lon.lock already exists");
        return Ok(());
    }

    if args.from.is_none() && args.source.is_none() {
        log::info!("Writing empty lon.lock...");
        let sources = Sources::default();
        sources.write(directory)?;
        return Ok(());
    }

    let Some(path) = &args.source else {
        bail!("No path to initialize from is provided");
    };

    let Some(lock_file_type) = &args.from else {
        bail!("No lock file type is provided");
    };

    log::info!("Initializing lon.lock from {}", path.display());

    let sources = match lock_file_type {
        LockFileType::Niv => niv::LockFile::from_file(path)?.convert()?,
        LockFileType::Npins => npins::LockFile::from_file(path)?.convert()?,
    };

    sources.write(&directory)?;

    Ok(())
}

fn add_git(directory: impl AsRef<Path>, args: &AddGitArgs) -> Result<()> {
    let mut sources = Sources::read(&directory)?;
    if sources.contains(&args.name) {
        bail!("Source {} already exists", args.name);
    }

    log::info!("Adding {}...", args.name);

    let source = GitSource::new(
        &args.url,
        args.branch.as_ref(),
        args.revision.as_ref(),
        args.submodules,
        args.frozen,
    )?;

    sources.add(&args.name, Source::Git(source));

    sources.write(&directory)?;
    LonNix::update(&directory)?;

    Ok(())
}

fn add_github(directory: impl AsRef<Path>, args: &AddGitHubArgs) -> Result<()> {
    let Some((owner, repo)) = args.identifier.split_once('/') else {
        bail!("Failed to parse identifier {}", args.identifier)
    };

    let name = args.name.clone().unwrap_or(repo.to_string());

    let mut sources = Sources::read(&directory)?;
    if sources.contains(&name) {
        bail!("Source {name} already exists");
    }

    log::info!("Adding {name}...");

    let source = GitHubSource::new(
        owner,
        repo,
        args.branch.as_ref(),
        args.revision.as_ref(),
        args.frozen,
    )?;

    sources.add(&name, Source::GitHub(source));

    sources.write(&directory)?;
    LonNix::update(&directory)?;

    Ok(())
}

fn add_tarball(directory: impl AsRef<Path>, args: &AddTarballArgs) -> Result<()> {
    let mut sources = Sources::read(&directory)?;
    if sources.contains(&args.name) {
        bail!("Source {} already exists", args.name);
    }

    log::info!("Adding {}...", args.name);

    let source = TarballSource::new(&args.url, args.frozen)?;

    sources.add(&args.name, Source::Tarball(source));

    sources.write(&directory)?;
    LonNix::update(&directory)?;

    Ok(())
}

fn update(directory: impl AsRef<Path>, args: &UpdateArgs) -> Result<()> {
    let mut sources = Sources::read(&directory)?;

    let mut names = Vec::new();

    if let Some(ref name) = args.name {
        names.push(name.clone());
    } else {
        names.extend(sources.names().into_iter().map(ToString::to_string));
    }

    if names.is_empty() {
        bail!("Lock file doesn't contain any sources")
    }

    let mut commit_message = CommitMessage::new();

    for name in &names {
        let Some(source) = sources.get_mut(name) else {
            bail!("Source {name} doesn't exist")
        };

        log::info!("Updating {name}...");

        let summary = match source
            .update()
            .with_context(|| format!("Failed to update {name}"))
        {
            Ok(summary) => summary,
            Err(error) => {
                if args.r#continue {
                    log::warn!("Skipping: {error}");
                    continue;
                }
                Err(error)?
            }
        };

        if let Some(summary) = summary {
            commit_message.add_summary(name, summary);
        }
    }

    if commit_message.is_empty() {
        bail!("No updates available")
    }

    sources.write(&directory)?;
    LonNix::update(&directory)?;

    if args.commit {
        commit(&directory, &commit_message.to_string(), None)?;
    }

    Ok(())
}

fn modify(directory: impl AsRef<Path>, args: &ModifyArgs) -> Result<()> {
    let mut sources = Sources::read(&directory)?;

    let Some(source) = sources.get_mut(&args.name) else {
        bail!("Source {} doesn't exist", args.name)
    };

    log::info!("Modifying {}...", args.name);

    source.modify(
        args.branch.as_ref(),
        args.revision.as_ref(),
        args.url.as_ref(),
    )?;

    sources.write(&directory)?;
    LonNix::update(&directory)?;

    Ok(())
}

fn remove(directory: impl AsRef<Path>, args: &SourceArgs) -> Result<()> {
    let mut sources = Sources::read(&directory)?;

    if !sources.contains(&args.name) {
        bail!("Source {} doesn't exist", args.name)
    }

    log::info!("Removing {}...", args.name);

    sources.remove(&args.name);

    sources.write(&directory)?;
    LonNix::update(&directory)?;

    Ok(())
}

fn freeze(directory: impl AsRef<Path>, args: &SourceArgs) -> Result<()> {
    let mut sources = Sources::read(&directory)?;

    let Some(source) = sources.get_mut(&args.name) else {
        bail!("Source {} doesn't exist", args.name)
    };

    log::info!("Freezing {}...", args.name);

    source.freeze();

    sources.write(&directory)?;
    LonNix::update(&directory)?;

    Ok(())
}

fn unfreeze(directory: impl AsRef<Path>, args: &SourceArgs) -> Result<()> {
    let mut sources = Sources::read(&directory)?;

    let Some(source) = sources.get_mut(&args.name) else {
        bail!("Source {} doesn't exist", args.name)
    };

    log::info!("Unfreezing {}...", args.name);

    source.unfreeze();

    sources.write(&directory)?;
    LonNix::update(&directory)?;

    Ok(())
}

fn show(directory: impl AsRef<Path>, name: Option<String>) -> Result<()> {
    let mut sources = Sources::read(&directory)?;

    match name {
        Some(name) => {
            let Some(source) = sources.get_mut(&name) else {
                bail!("Source {name} doesn't exist")
            };

            println!("{name}:\n{source}");
        }
        None => {
            for (name, source) in sources.iter() {
                println!("{name}:\n{source}");
            }
        }
    }

    Ok(())
}

fn bot(directory: impl AsRef<Path>, forge: &impl Forge) -> Result<()> {
    let base_ref = git::current_rev(&directory)?;

    let result = bot_fallible(&directory, forge, &base_ref);

    // Always return to the base commit.
    git::checkout(&directory, &base_ref, false)?;

    result
}

fn bot_fallible(directory: impl AsRef<Path>, forge: &impl Forge, base_ref: &str) -> Result<()> {
    let sources = Sources::read(&directory)?;

    let names = sources
        .names()
        .into_iter()
        .cloned()
        .collect::<Vec<String>>();

    let list_commits = match env::var("LON_LIST_COMMITS") {
        Ok(s) => s.parse::<usize>().unwrap_or(50),
        Err(_) => 0,
    };

    for name in &names {
        // Clone the original sources to reset the state between updates
        let mut m_sources = sources.clone();

        let Some(source) = m_sources.get_mut(name) else {
            log::warn!("Source {name} doesn't exist");
            continue;
        };

        if source.frozen() {
            log::info!("Source {name} is frozen. Skipping...");
            continue;
        }

        log::debug!("Checking out base ref {base_ref}...");
        git::checkout(&directory, base_ref, false)?;

        let branch = format!("lon/{name}");
        log::debug!("Checking out new branch {branch}...");
        git::checkout(&directory, &branch, true)?;

        log::info!("Updating {name}...");

        let summary = source
            .update()
            .with_context(|| format!("Failed to update {name}"))?;

        let Some(mut summary) = summary else {
            log::info!("No updates available");
            continue;
        };

        if let UpdateSummary::Rev(ref mut summary) = summary
            && list_commits > 0
            && let Some(rev_list) = source.rev_list(summary, list_commits)?
        {
            summary.add_rev_list(rev_list);
        }

        let mut commit_message = CommitMessage::new();

        commit_message.add_summary(name, summary.clone());

        m_sources.write(&directory)?;
        LonNix::update(&directory)?;

        let user_name = env::var("LON_USER_NAME").unwrap_or("LonBot".into());
        let user_email = env::var("LON_USER_EMAIL").unwrap_or("lonbot@lonbot".into());

        log::debug!("Committing changes...");
        commit(
            &directory,
            &commit_message.to_string(),
            Some(git::User::new(&user_name, &user_email)),
        )?;

        let push_url = env::var("LON_PUSH_URL").ok();

        // Never log the URL as it might contain a secret token.
        log::debug!("Force pushing repository...");
        git::force_push(&directory, push_url.as_deref(), &branch)?;

        match forge.open_pull_request(&branch, name, Some(commit_message.body()?)) {
            Ok(pull_request_url) => log::info!("Opened Pull Request: {pull_request_url}"),
            Err(err) => log::warn!("{err}"),
        }
    }

    Ok(())
}

fn commit(
    directory: impl AsRef<Path>,
    commit_message: &str,
    user: Option<git::User>,
) -> Result<()> {
    // Don't provide the directory twice. The `git add` command is already executed in the
    // directory, so the Lock and LonNix paths don't need to include it as well.
    git::add(&directory, &[&Lock::path(""), &LonNix::path("")])?;
    git::commit(&directory, commit_message, user)?;
    Ok(())
}
