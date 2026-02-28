use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd() -> assert_cmd::Command {
    let mut c = cargo_bin_cmd!("jigolo");
    c.arg("--list");
    c
}

#[test]
fn no_args_searches_current_directory() {
    cmd().assert().success();
}

#[test]
fn help_flag_succeeds() {
    cargo_bin_cmd!("jigolo")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("CLAUDE.md"));
}

#[test]
fn version_flag_succeeds() {
    cargo_bin_cmd!("jigolo")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("jigolo"));
}

#[test]
fn nonexistent_path_exits_with_code_1() {
    cmd()
        .arg("/nonexistent/path/that/does/not/exist")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Warning"));
}

#[test]
fn finds_claude_md_in_temp_dir() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("CLAUDE.md"), "test content").unwrap();

    cmd()
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("1 file"));
}

#[test]
fn no_claude_files_prints_friendly_message() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("README.md"), "not claude").unwrap();

    cmd()
        .env("HOME", tmp.path())
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No CLAUDE.md files found."));
}

#[test]
fn file_path_argument_warns_and_fails() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("somefile.txt");
    std::fs::write(&file, "content").unwrap();

    cmd()
        .arg(&file)
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("not a directory"));
}

#[test]
fn mixed_valid_and_invalid_paths_still_succeeds() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("CLAUDE.md"), "test").unwrap();

    cmd()
        .arg(tmp.path())
        .arg("/nonexistent/path")
        .assert()
        .success()
        .stdout(predicate::str::contains("1 file"))
        .stderr(predicate::str::contains("Warning"));
}
