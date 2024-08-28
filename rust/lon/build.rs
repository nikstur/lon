use std::{
    env,
    fs::{self, File},
    io,
};

use sha2::{Digest, Sha256};

const LON_NIX_FILENAME: &str = "lon.nix";

fn main() {
    let mut out_path = env::var_os("OUT_DIR").expect("Failed to read OUT_DIR");
    out_path.push(format!("{LON_NIX_FILENAME}.sha256"));

    let mut file = File::open(format!("src/{LON_NIX_FILENAME}")).expect("Failed to read lon.nix");

    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher).expect("Failed to hash lon.nix");
    let hash = hasher.finalize();

    fs::write(out_path, hash).expect("Failed to write lon.nix.sha256");
}
