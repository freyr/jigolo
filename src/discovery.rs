use std::env;
use std::path::Path;
use std::path::PathBuf;

use walkdir::DirEntry;
use walkdir::WalkDir;

/// Directories that will never contain CLAUDE.md files.
/// Using `filter_entry()` prunes entire subtrees — this is the critical
/// performance optimisation. Without it, scanning a home directory with
/// JS projects can take 30-60 seconds instead of <1 second.
pub const SKIP_DIRS: &[&str] = &[
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

pub fn should_descend(entry: &DirEntry) -> bool {
    if entry.file_type().is_dir() {
        let name = entry.file_name().to_string_lossy();
        return !SKIP_DIRS.iter().any(|d| *d == name.as_ref());
    }
    true
}

pub fn find_global_claude_file() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    find_global_claude_file_in(&PathBuf::from(home))
}

pub fn find_global_claude_file_in(home: &Path) -> Option<PathBuf> {
    let path = home.join(".claude").join("CLAUDE.md");
    path.exists().then_some(path)
}

/// Default maximum directory depth for scanning.
pub const DEFAULT_MAX_DEPTH: usize = 3;

/// Finds all `CLAUDE.md` files under `root`, up to `max_depth` levels deep.
///
/// Silently skips broken symlinks, permission errors, and other IO failures.
pub fn find_claude_files(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(root)
        .follow_links(true)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(should_descend)
        .filter_map(|result| result.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name() == "CLAUDE.md")
        .map(|entry| entry.into_path())
        .collect();

    files.sort_unstable();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn finds_claude_md_in_nested_dirs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("sub/deep")).unwrap();
        fs::write(root.join("CLAUDE.md"), "root").unwrap();
        fs::write(root.join("sub/CLAUDE.md"), "sub").unwrap();
        fs::write(root.join("sub/deep/CLAUDE.md"), "deep").unwrap();
        fs::write(root.join("sub/not-claude.md"), "ignored").unwrap();

        let files = find_claude_files(root, 10);

        assert_eq!(files.len(), 3);
        assert!(files.iter().all(|f| f.file_name().unwrap() == "CLAUDE.md"));
    }

    #[test]
    fn returns_empty_for_no_claude_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("README.md"), "not claude").unwrap();

        let files = find_claude_files(tmp.path(), DEFAULT_MAX_DEPTH);

        assert!(files.is_empty());
    }

    #[test]
    fn skips_filtered_directories() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("node_modules/deep")).unwrap();
        fs::write(root.join("node_modules/deep/CLAUDE.md"), "skip").unwrap();
        fs::write(root.join("CLAUDE.md"), "keep").unwrap();

        let files = find_claude_files(root, DEFAULT_MAX_DEPTH);

        assert_eq!(files.len(), 1);
    }

    #[test]
    fn find_global_claude_file_returns_path_when_exists() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".claude")).unwrap();
        fs::write(tmp.path().join(".claude/CLAUDE.md"), "global").unwrap();

        let result = find_global_claude_file_in(tmp.path());

        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            tmp.path().join(".claude").join("CLAUDE.md")
        );
    }

    #[test]
    fn find_global_claude_file_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();

        let result = find_global_claude_file_in(tmp.path());

        assert!(result.is_none());
    }

    #[test]
    fn default_depth_limits_to_three_levels() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Level 1: root/CLAUDE.md
        fs::write(root.join("CLAUDE.md"), "level 0").unwrap();
        // Level 2: root/a/CLAUDE.md
        fs::create_dir_all(root.join("a")).unwrap();
        fs::write(root.join("a/CLAUDE.md"), "level 1").unwrap();
        // Level 3: root/a/b/CLAUDE.md
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("a/b/CLAUDE.md"), "level 2").unwrap();
        // Level 4: root/a/b/c/CLAUDE.md — should be excluded at default depth 3
        fs::create_dir_all(root.join("a/b/c")).unwrap();
        fs::write(root.join("a/b/c/CLAUDE.md"), "level 3").unwrap();

        let files = find_claude_files(root, DEFAULT_MAX_DEPTH);

        assert_eq!(files.len(), 3, "Default depth 3 should find 3 files, not 4");
    }

    #[test]
    fn custom_depth_overrides_default() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("a/b/c")).unwrap();
        fs::write(root.join("CLAUDE.md"), "level 0").unwrap();
        fs::write(root.join("a/CLAUDE.md"), "level 1").unwrap();
        fs::write(root.join("a/b/CLAUDE.md"), "level 2").unwrap();
        fs::write(root.join("a/b/c/CLAUDE.md"), "level 3").unwrap();

        let files = find_claude_files(root, 10);

        assert_eq!(files.len(), 4, "Depth 10 should find all 4 files");
    }

    #[test]
    fn broken_symlinks_are_silently_skipped() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::write(root.join("CLAUDE.md"), "keep").unwrap();

        // Create a symlink pointing to a nonexistent target
        #[cfg(unix)]
        std::os::unix::fs::symlink("/nonexistent/target", root.join("broken_link")).unwrap();

        let files = find_claude_files(root, DEFAULT_MAX_DEPTH);

        assert_eq!(files.len(), 1, "Broken symlinks should be silently skipped");
    }

    #[test]
    fn results_are_sorted() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("z-dir")).unwrap();
        fs::create_dir_all(root.join("a-dir")).unwrap();
        fs::write(root.join("z-dir/CLAUDE.md"), "z").unwrap();
        fs::write(root.join("a-dir/CLAUDE.md"), "a").unwrap();

        let files = find_claude_files(root, DEFAULT_MAX_DEPTH);

        assert_eq!(files.len(), 2);
        assert!(
            files[0] < files[1],
            "Results should be sorted alphabetically."
        );
    }
}
