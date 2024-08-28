use anyhow::Result;
use tempfile::tempdir;

use crate::init;

#[test]
fn create_files() -> Result<()> {
    let tmpdir = tempdir()?;

    init(tmpdir.path())?;

    assert!(tmpdir.path().join("lon.nix").exists());
    assert!(tmpdir.path().join("lon.lock").exists());

    Ok(())
}
