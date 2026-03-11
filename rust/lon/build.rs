use std::{
    env,
    fs::{self},
};

use sha2::{Digest, Sha256};

const LON_NIX_FILENAME: &str = "lon.nix";

fn main() {
    let mut out_path = env::var_os("OUT_DIR").expect("Failed to read OUT_DIR");
    out_path.push(format!("{LON_NIX_FILENAME}.sha256"));

    let content = fs::read(format!("src/{LON_NIX_FILENAME}")).expect("Failed to read lon.nix");
    let hash = Sha256::digest(content);

    fs::write(out_path, hash).expect("Failed to write lon.nix.sha256");
}
