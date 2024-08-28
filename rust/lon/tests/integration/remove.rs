use std::{
    fs::{self, File},
    path::Path,
};

use anyhow::{Context, Result};
use expect_test::expect;
use tempfile::tempdir;

use crate::{init, lon};

fn mock_lock(tmpdir: &Path) -> Result<()> {
    let path = tmpdir.join("lon.lock");

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
            },
            "lanzaboote": {
                "type": "Git",
                "fetchType": "git",
                "branch": "master",
                "revision": "f5a3a7dff44d131807fc1a89fbd8576cd870334a",
                "url": "git@github.com:nix-community/lanzaboote.git",
                "hash": "sha256-e/fSi0WER06N8WCvpht62fkGtWfe5ckDxr6zNYkwkFw=",
            },
        }
    });

    let mut file = File::create(&path).with_context(|| format!("Failed to open {:?}", &path))?;
    serde_json::to_writer_pretty(&mut file, &value).context("Failed to serialize lock file")?;

    Ok(())
}

#[test]
fn remove() -> Result<()> {
    let tmpdir = tempdir()?;

    init(tmpdir.path())?;
    mock_lock(tmpdir.path())?;

    let output0 = lon(tmpdir.path(), ["remove", "nixpkgs"])?;
    assert!(output0.status.success());

    let lock_path = tmpdir.path().join("lon.lock");

    let actual = fs::read_to_string(&lock_path)?;
    let expected = expect![[r#"
        {
          "version": "1",
          "sources": {
            "lanzaboote": {
              "type": "Git",
              "fetchType": "git",
              "branch": "master",
              "revision": "f5a3a7dff44d131807fc1a89fbd8576cd870334a",
              "url": "git@github.com:nix-community/lanzaboote.git",
              "hash": "sha256-e/fSi0WER06N8WCvpht62fkGtWfe5ckDxr6zNYkwkFw=",
              "submodules": false
            }
          }
        }
    "#]];
    expected.assert_eq(&actual);

    let output1 = lon(tmpdir.path(), ["remove", "lanzaboote"])?;
    assert!(output1.status.success());

    let actual = fs::read_to_string(lock_path)?;
    let expected = expect![[r#"
        {
          "version": "1",
          "sources": {}
        }
    "#]];
    expected.assert_eq(&actual);

    let output2 = lon(tmpdir.path(), ["remove", "lanzaboote"])?;
    assert!(!output2.status.success());

    Ok(())
}
