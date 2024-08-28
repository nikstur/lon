mod cli;
mod git;
mod lock;
mod lon_nix;
mod nix;
mod sources;

use std::process::ExitCode;

use cli::Cli;

fn main() -> ExitCode {
    Cli::init(module_path!())
}
