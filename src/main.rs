use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;

/// A TUI for managing Claude Code context files
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Directories to search for CLAUDE.md files
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
}

fn list_directory(path: &Path) -> Result<()> {
    let entries = fs::read_dir(path)
        .with_context(|| format!("Failed to read directory: {}", path.display()))?;

    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let prefix = if file_type.is_dir() { "d" } else { "f" };
        println!("  [{}] {}", prefix, entry.file_name().to_string_lossy());
    }

    Ok(())
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    for path in &cli.paths {
        println!("{}:", path.display());
        list_directory(path)?;
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {:#}", err);
        process::exit(1);
    }
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
