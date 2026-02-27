use std::path::PathBuf;

use clap::Parser;

/// A TUI for managing Claude Code context files
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Directories to search for CLAUDE.md files
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    println!("Searching in: {:?}", cli.paths);
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }
}
