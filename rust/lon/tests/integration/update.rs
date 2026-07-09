use std::fs;
use std::process::Output;

use anyhow::Result;
use expect_test::expect;
use tempfile::tempdir;

use crate::{add_local_repo_commit, init, init_local_repo, lon};

#[test]
fn update_local() -> Result<()> {
    let mut output: Output;

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

    add_local_repo_commit(git_tmpdir.path())?;

    output = lon(lon_tmpdir.path(), ["update"])?;
    assert!(output.status.success(), "Failed to update mock repo");

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
              "revision": "11b4a64fdd67e6f6cef00027cf336e7ccd8332b8",
              dummy,
              "hash": "sha256-vp2y6czq91Qdb1fG3gj3mEFNvv05GoR9aWWl2fxKz3E=",
              "lastModified": 1783461600,
              "submodules": false
            }
          }
        }
    "#]];

    expected.assert_eq(&actual_filtered_url);
    Ok(())
}
