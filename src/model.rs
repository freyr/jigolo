use std::fmt;
use std::path::PathBuf;

use clap::Parser;

/// A TUI for managing Claude Code context files
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    /// Directories to search for CLAUDE.md files
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// List files and exit (no TUI)
    #[arg(long)]
    pub list: bool,
}

/// One of the root directories provided by the user, with all CLAUDE.md files found within it.
#[derive(Debug, Clone)]
pub struct SourceRoot {
    /// The root directory path (as provided by the user)
    pub path: PathBuf,
    /// Full paths to all discovered CLAUDE.md files within this root
    pub files: Vec<PathBuf>,
}

impl SourceRoot {
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

impl fmt::Display for SourceRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.file_count();
        let label = if count == 1 { "file" } else { "files" };
        writeln!(f, "{} ({} {})", self.path.display(), count, label)?;
        for file in &self.files {
            let relative = file.strip_prefix(&self.path).unwrap_or(file);
            writeln!(f, "  {}", relative.display())?;
        }
        Ok(())
    }
}

/// Return value from run() — keeps all process::exit() calls in main().
#[derive(Debug)]
pub enum ExitOutcome {
    Success,
    AllPathsFailed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    fn source_root_display_shows_count_and_relative_paths() {
        let root = SourceRoot {
            path: PathBuf::from("/tmp/test"),
            files: vec![
                PathBuf::from("/tmp/test/CLAUDE.md"),
                PathBuf::from("/tmp/test/sub/CLAUDE.md"),
            ],
        };
        let output = format!("{root}");
        assert!(output.contains("2 files"));
        assert!(output.contains("CLAUDE.md"));
        assert!(output.contains("sub/CLAUDE.md"));
    }

    #[test]
    fn source_root_display_singular_file() {
        let root = SourceRoot {
            path: PathBuf::from("/tmp/test"),
            files: vec![PathBuf::from("/tmp/test/CLAUDE.md")],
        };
        let output = format!("{root}");
        assert!(output.contains("1 file)"));
    }
}
