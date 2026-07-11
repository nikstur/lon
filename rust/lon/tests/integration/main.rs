use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use anyhow::{Result, bail};
use assert_cmd;

mod add;
mod ignored;
mod init;
mod remove;
mod update;

pub fn lon<'a>(tmpdir: &Path, args: impl IntoIterator<Item = &'a str>) -> Result<Output> {
    let mut cmd = assert_cmd::Command::cargo_bin("lon")?;
    let output = cmd
        .arg("-vv")
        .arg("--directory")
        .arg(tmpdir)
        .args(args)
        .output()?;

    // Print debugging output.
    // This is a weird hack to make cargo test capture the output.
    // See https://github.com/rust-lang/rust/issues/12309
    print!("{}", String::from_utf8(output.stdout.clone())?);
    print!("{}", String::from_utf8(output.stderr.clone())?);

    Ok(output)
}

fn init(tmpdir: &Path) -> Result<Output> {
    let output = lon(tmpdir, ["init"])?;
    if !output.status.success() {
        bail!("Failed to init lon");
    }
    Ok(output)
}

fn init_local_repo(tmpdir: &Path) -> Result<()> {
    let mut output: Output;

    output = Command::new("git")
        .arg("-C")
        .arg(tmpdir)
        .arg("init")
        .arg("-b")
        .arg("main")
        .output()?;

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let test_file_path = tmpdir.join("test.txt");
    fs::write(&test_file_path, "test file contents")?;

    output = Command::new("git")
        .arg("-C")
        .arg(tmpdir)
        .arg("add")
        .arg(test_file_path)
        .output()?;

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    output = Command::new("git")
        .env("GIT_CONFIG_GLOBAL", "")
        .env("GIT_CONFIG_SYSTEM", "")
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_DATE", "Wed Jul 8 00:00:00 2026 +0200")
        .env("GIT_AUTHOR_DATE", "Wed Jul 8 00:00:00 2026 +0200")
        .arg("-C")
        .arg(tmpdir)
        .arg("commit")
        .arg("-m")
        .arg("'test commit msg'")
        .output()?;

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

fn add_local_repo_commit(tmpdir: &Path) -> Result<()> {
    let mut output: Output;

    let test_file_path = tmpdir.join("test2.txt");
    fs::write(&test_file_path, "test file num 2 contents")?;

    output = Command::new("git")
        .arg("-C")
        .arg(tmpdir)
        .arg("add")
        .arg(test_file_path)
        .output()?;

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    output = Command::new("git")
        .env("GIT_CONFIG_GLOBAL", "")
        .env("GIT_CONFIG_SYSTEM", "")
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_DATE", "Wed Jul 8 00:00:00 2026 +0200")
        .env("GIT_AUTHOR_DATE", "Wed Jul 8 00:00:00 2026 +0200")
        .arg("-C")
        .arg(tmpdir)
        .arg("commit")
        .arg("-m")
        .arg("'test commit 2 msg'")
        .output()?;

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}
