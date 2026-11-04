//! `uniformat`: command line of the UniFormat family (see `uniformat --help`).
use std::process::ExitCode;

fn main() -> ExitCode {
    uniformat_cli::run(std::env::args_os())
}
