//! The `palmrest` command: the daemon, and its record / replay / probe tools.

use std::process::ExitCode;

fn main() -> ExitCode {
    palmrest::command::run(std::env::args())
}
