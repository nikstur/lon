use std::{path::Path, process::Output};

use anyhow::{bail, Result};
use assert_cmd::Command;

mod init;
mod remove;

pub fn lon(tmpdir: &Path, args: impl IntoIterator<Item = &'static str>) -> Result<Output> {
    let mut cmd = Command::cargo_bin("lon")?;
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
