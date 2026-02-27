use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::process;

use clap::Parser;
use walkdir::DirEntry;
use walkdir::WalkDir;

/// A TUI for managing Claude Code context files
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Directories to search for CLAUDE.md files
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
}

/// One of the root directories provided by the user, with all CLAUDE.md files found within it.
#[derive(Debug, Clone)]
struct SourceRoot {
    /// The root directory path (as provided by the user)
    path: PathBuf,
    /// Full paths to all discovered CLAUDE.md files within this root
    files: Vec<PathBuf>,
}

impl SourceRoot {
    fn file_count(&self) -> usize {
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
enum ExitOutcome {
    Success,
    AllPathsFailed,
}

/// Directories that will never contain CLAUDE.md files.
/// Using `filter_entry()` prunes entire subtrees — this is the critical
/// performance optimisation. Without it, scanning a home directory with
/// JS projects can take 30-60 seconds instead of <1 second.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    ".cache",
    "__pycache__",
    ".venv",
    "vendor",
    "dist",
    ".next",
    ".nuxt",
    "build",
];

fn should_descend(entry: &DirEntry) -> bool {
    if entry.file_type().is_dir() {
        let name = entry.file_name().to_string_lossy();
        return !SKIP_DIRS.iter().any(|d| *d == name.as_ref());
    }
    true
}

fn find_claude_files(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(root)
        .follow_links(true)
        .max_depth(100)
        .into_iter()
        .filter_entry(should_descend)
        .filter_map(|result| match result {
            Ok(entry) => Some(entry),
            Err(err) => {
                eprintln!(
                    "Warning: {}: {}",
                    err.path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "<unknown>".into()),
                    err
                );
                None
            }
        })
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name() == "CLAUDE.md")
        .map(|entry| entry.into_path())
        .collect();

    files.sort_unstable();
    files
}

fn run() -> ExitOutcome {
    let cli = Cli::parse();

    let mut roots: Vec<SourceRoot> = Vec::new();
    let mut failed_count: usize = 0;

    eprintln!(
        "Scanning {} {}...",
        cli.paths.len(),
        if cli.paths.len() == 1 {
            "directory"
        } else {
            "directories"
        }
    );

    for path in &cli.paths {
        if !path.exists() {
            eprintln!("Warning: path does not exist: {}", path.display());
            failed_count += 1;
            continue;
        }
        if !path.is_dir() {
            eprintln!("Warning: not a directory: {}", path.display());
            failed_count += 1;
            continue;
        }

        let files = find_claude_files(path);
        roots.push(SourceRoot {
            path: path.clone(),
            files,
        });
    }

    if roots.is_empty() && failed_count > 0 {
        return ExitOutcome::AllPathsFailed;
    }

    let total: usize = roots.iter().map(|r| r.file_count()).sum();

    if total == 0 {
        println!("No CLAUDE.md files found.");
    } else {
        for root in &roots {
            println!();
            print!("{root}");
        }
        println!(
            "Found {} CLAUDE.md {} in {} {}.",
            total,
            if total == 1 { "file" } else { "files" },
            roots.len(),
            if roots.len() == 1 {
                "directory"
            } else {
                "directories"
            }
        );
    }

    ExitOutcome::Success
}

fn main() {
    match run() {
        ExitOutcome::Success => {}
        ExitOutcome::AllPathsFailed => process::exit(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    fn finds_claude_md_in_nested_dirs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("sub/deep")).unwrap();
        fs::write(root.join("CLAUDE.md"), "root").unwrap();
        fs::write(root.join("sub/CLAUDE.md"), "sub").unwrap();
        fs::write(root.join("sub/deep/CLAUDE.md"), "deep").unwrap();
        fs::write(root.join("sub/not-claude.md"), "ignored").unwrap();

        let files = find_claude_files(root);

        assert_eq!(files.len(), 3);
        assert!(files.iter().all(|f| f.file_name().unwrap() == "CLAUDE.md"));
    }

    #[test]
    fn returns_empty_for_no_claude_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("README.md"), "not claude").unwrap();

        let files = find_claude_files(tmp.path());

        assert!(files.is_empty());
    }

    #[test]
    fn skips_filtered_directories() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("node_modules/deep")).unwrap();
        fs::write(root.join("node_modules/deep/CLAUDE.md"), "skip").unwrap();
        fs::write(root.join("CLAUDE.md"), "keep").unwrap();

        let files = find_claude_files(root);

        assert_eq!(files.len(), 1);
    }

    #[test]
    fn results_are_sorted() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("z-dir")).unwrap();
        fs::create_dir_all(root.join("a-dir")).unwrap();
        fs::write(root.join("z-dir/CLAUDE.md"), "z").unwrap();
        fs::write(root.join("a-dir/CLAUDE.md"), "a").unwrap();

        let files = find_claude_files(root);

        assert_eq!(files.len(), 2);
        assert!(
            files[0] < files[1],
            "Results should be sorted alphabetically"
        );
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
