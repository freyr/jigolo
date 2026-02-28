use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snippet {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnippetLibrary {
    #[serde(default)]
    pub snippets: Vec<Snippet>,
}

pub fn library_path() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    Some(library_path_in(&PathBuf::from(home)))
}

pub fn library_path_in(home: &Path) -> PathBuf {
    home.join(".config")
        .join("jigolo")
        .join("library.toml")
}

pub fn load_library(path: &Path) -> Result<SnippetLibrary> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let lib: SnippetLibrary = toml::from_str(&contents)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            Ok(lib)
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(SnippetLibrary::default()),
        Err(err) => Err(anyhow::anyhow!(
            "failed to read {}: {}",
            path.display(),
            err
        )),
    }
}

pub fn save_library(lib: &SnippetLibrary, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    let contents = toml::to_string_pretty(lib).context("failed to serialize library")?;
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn append_snippet(snippet: Snippet, path: &Path) -> Result<()> {
    let mut lib = load_library(path)?;
    lib.snippets.push(snippet);
    save_library(&lib, path)
}

pub fn delete_snippet(index: usize, path: &Path) -> Result<()> {
    let mut lib = load_library(path)?;
    if index < lib.snippets.len() {
        lib.snippets.remove(index);
        save_library(&lib, path)?;
    }
    Ok(())
}

pub fn rename_snippet(index: usize, new_title: &str, path: &Path) -> Result<()> {
    let mut lib = load_library(path)?;
    if index < lib.snippets.len() {
        lib.snippets[index].title = new_title.to_string();
        save_library(&lib, path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_snippet(title: &str) -> Snippet {
        Snippet {
            title: title.to_string(),
            content: "some content".to_string(),
            source: "/path/to/CLAUDE.md".to_string(),
        }
    }

    #[test]
    fn round_trip_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("library.toml");

        let lib = SnippetLibrary {
            snippets: vec![sample_snippet("Test Snippet")],
        };

        save_library(&lib, &path).unwrap();
        let loaded = load_library(&path).unwrap();

        assert_eq!(loaded, lib);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.toml");

        let lib = load_library(&path).unwrap();

        assert!(lib.snippets.is_empty());
    }

    #[test]
    fn load_invalid_toml_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("bad.toml");
        fs::write(&path, "this is not valid [[[ toml").unwrap();

        let result = load_library(&path);

        assert!(result.is_err());
    }

    #[test]
    fn save_creates_parent_directories() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("deep").join("nested").join("library.toml");

        let lib = SnippetLibrary::default();
        save_library(&lib, &path).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn append_snippet_adds_to_existing_library() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("library.toml");

        append_snippet(sample_snippet("First"), &path).unwrap();
        append_snippet(sample_snippet("Second"), &path).unwrap();

        let lib = load_library(&path).unwrap();
        assert_eq!(lib.snippets.len(), 2);
        assert_eq!(lib.snippets[0].title, "First");
        assert_eq!(lib.snippets[1].title, "Second");
    }

    #[test]
    fn append_snippet_to_nonexistent_file_creates_it() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("new_library.toml");

        append_snippet(sample_snippet("Solo"), &path).unwrap();

        let lib = load_library(&path).unwrap();
        assert_eq!(lib.snippets.len(), 1);
        assert_eq!(lib.snippets[0].title, "Solo");
    }

    #[test]
    fn snippet_without_source_deserializes_with_default() {
        let toml_str = r#"
[[snippets]]
title = "No source"
content = "body"
"#;
        let lib: SnippetLibrary = toml::from_str(toml_str).unwrap();

        assert_eq!(lib.snippets[0].source, "");
    }

    #[test]
    fn empty_library_serializes_cleanly() {
        let lib = SnippetLibrary::default();
        let output = toml::to_string_pretty(&lib).unwrap();

        assert_eq!(output.trim(), "snippets = []");
    }

    #[test]
    fn library_path_resolves_from_home() {
        let tmp = TempDir::new().unwrap();

        let path = library_path_in(tmp.path());

        let expected = tmp
            .path()
            .join(".config")
            .join("jigolo")
            .join("library.toml");
        assert_eq!(path, expected);
    }

    #[test]
    fn delete_snippet_removes_by_index() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("library.toml");

        append_snippet(sample_snippet("First"), &path).unwrap();
        append_snippet(sample_snippet("Second"), &path).unwrap();
        append_snippet(sample_snippet("Third"), &path).unwrap();

        delete_snippet(1, &path).unwrap();

        let lib = load_library(&path).unwrap();
        assert_eq!(lib.snippets.len(), 2);
        assert_eq!(lib.snippets[0].title, "First");
        assert_eq!(lib.snippets[1].title, "Third");
    }

    #[test]
    fn delete_snippet_out_of_bounds_is_noop() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("library.toml");

        append_snippet(sample_snippet("Only"), &path).unwrap();

        delete_snippet(5, &path).unwrap();

        let lib = load_library(&path).unwrap();
        assert_eq!(lib.snippets.len(), 1);
    }

    #[test]
    fn delete_snippet_from_single_item_library() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("library.toml");

        append_snippet(sample_snippet("Solo"), &path).unwrap();

        delete_snippet(0, &path).unwrap();

        let lib = load_library(&path).unwrap();
        assert!(lib.snippets.is_empty());
    }

    #[test]
    fn rename_snippet_changes_title() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("library.toml");

        append_snippet(sample_snippet("Old Name"), &path).unwrap();

        rename_snippet(0, "New Name", &path).unwrap();

        let lib = load_library(&path).unwrap();
        assert_eq!(lib.snippets[0].title, "New Name");
    }

    #[test]
    fn rename_snippet_out_of_bounds_is_noop() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("library.toml");

        append_snippet(sample_snippet("Only"), &path).unwrap();

        rename_snippet(5, "Nope", &path).unwrap();

        let lib = load_library(&path).unwrap();
        assert_eq!(lib.snippets[0].title, "Only");
    }
}
