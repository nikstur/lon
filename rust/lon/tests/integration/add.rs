use std::fs;
use std::process::Output;

use anyhow::Result;
use expect_test::expect;
use tempfile::tempdir;

use crate::{init, init_local_repo, lon};

#[test]
fn add_local() -> Result<()> {
    let output: Output;

    let git_tmpdir = tempdir()?;
    init_local_repo(git_tmpdir.path())?;

    let lon_tmpdir = tempdir()?;
    init(lon_tmpdir.path())?;

    let git_tmpdir_str = git_tmpdir.path().to_string_lossy();
    output = lon(
        lon_tmpdir.path(),
        ["add", "git", "repo", &git_tmpdir_str, "main"],
    )?;
    assert!(output.status.success(), "Failed to add repo");

    let lock_path = lon_tmpdir.path().join("lon.lock");

    let actual = fs::read_to_string(lock_path)?;

    let url_line = format!("\"url\": \"{git_tmpdir_str}\"");
    let actual_filtered_url = actual.to_string().replace(&url_line, "dummy");

    let expected = expect![[r#"
        {
          "version": "1",
          "sources": {
            "repo": {
              "type": "Git",
              "fetchType": "git",
              "branch": "main",
              "revision": "1110e91bd28c6dc11337bbe126d6e0f4e3302e8b",
              dummy,
              "hash": "sha256-zI+2B/s189+M/jdxG4JvQMNl1kwr8LD/ehwrdnLgAi4=",
              "lastModified": 1783461600,
              "submodules": false
            }
          }
        }
    "#]];

    expected.assert_eq(&actual_filtered_url);

    Ok(())
}
