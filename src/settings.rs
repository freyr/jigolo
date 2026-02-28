use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug)]
pub struct SettingsFile {
    pub label: String,
    pub path: PathBuf,
    pub value: serde_json::Value,
}

#[derive(Debug, Default)]
pub struct SettingsCollection {
    pub files: Vec<SettingsFile>,
}

/// Discover settings files using an explicit home directory (for testability).
pub fn discover_settings_files_in(home: Option<&Path>, project: &Path) -> SettingsCollection {
    let mut files = Vec::new();

    // 1. Global: ~/.claude/settings.json
    if let Some(home_dir) = home {
        let global_path = home_dir.join(".claude").join("settings.json");
        if let Some(sf) = load_settings_file("Global", &global_path) {
            files.push(sf);
        }
    }

    // 2. Project: .claude/settings.json
    let project_path = project.join(".claude").join("settings.json");
    if let Some(sf) = load_settings_file("Project", &project_path) {
        files.push(sf);
    }

    // 3. Project Local: .claude/settings.local.json
    let local_path = project.join(".claude").join("settings.local.json");
    if let Some(sf) = load_settings_file("Project Local", &local_path) {
        files.push(sf);
    }

    SettingsCollection { files }
}

/// Public wrapper that reads HOME from the environment.
pub fn discover_settings_files(project: &Path) -> SettingsCollection {
    let home = std::env::var("HOME").ok().map(PathBuf::from);
    discover_settings_files_in(home.as_deref(), project)
}

fn load_settings_file(label: &str, path: &Path) -> Option<SettingsFile> {
    let content = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => {
            return Some(SettingsFile {
                label: label.to_string(),
                path: path.to_path_buf(),
                value: serde_json::Value::String(format!("(invalid JSON: {})", path.display())),
            });
        }
    };
    Some(SettingsFile {
        label: label.to_string(),
        path: path.to_path_buf(),
        value,
    })
}

/// Format settings collection into display lines for the TUI.
pub fn format_settings(collection: &SettingsCollection) -> Vec<String> {
    let mut lines = Vec::new();

    for (i, file) in collection.files.iter().enumerate() {
        if i > 0 {
            lines.push(String::new());
        }

        // Section header
        lines.push(format!("▾ {} ({})", file.label, file.path.display()));

        // If the value is an error string, just show it
        if let serde_json::Value::String(s) = &file.value {
            lines.push(format!("  {s}"));
            continue;
        }

        let obj = match file.value.as_object() {
            Some(obj) => obj,
            None => {
                lines.push("  (not a JSON object)".to_string());
                continue;
            }
        };

        // Display in a specific order, then catch remaining keys
        let ordered_keys = [
            "model",
            "defaultMode",
            "thinking",
            "permissions",
            "mcpServers",
            "hooks",
            "plugins",
            "env",
        ];

        for &key in &ordered_keys {
            if let Some(val) = obj.get(key) {
                format_key_value(key, val, &mut lines);
            }
        }

        // Remaining keys not in the ordered list
        for (key, val) in obj {
            if !ordered_keys.contains(&key.as_str()) {
                format_key_value(key, val, &mut lines);
            }
        }
    }

    lines
}

fn format_key_value(key: &str, val: &serde_json::Value, lines: &mut Vec<String>) {
    match key {
        "model" => {
            lines.push(format!("  Model: {}", display_scalar(val)));
        }
        "defaultMode" => {
            lines.push(format!("  Default Mode: {}", display_scalar(val)));
        }
        "thinking" => {
            lines.push(format!("  Thinking: {}", display_scalar(val)));
        }
        "permissions" => {
            format_permissions(val, lines);
        }
        "mcpServers" => {
            format_mcp_servers(val, lines);
        }
        "hooks" => {
            format_hooks(val, lines);
        }
        "plugins" => {
            format_plugins(val, lines);
        }
        "env" => {
            format_env(val, lines);
        }
        _ => {
            lines.push(format!("  {key}: {}", format_inline(val)));
        }
    }
}

fn display_scalar(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

fn format_inline(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(display_scalar).collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::Object(_) => val.to_string(),
    }
}

fn format_permissions(val: &serde_json::Value, lines: &mut Vec<String>) {
    let obj = match val.as_object() {
        Some(o) => o,
        None => {
            lines.push(format!("  Permissions: {}", format_inline(val)));
            return;
        }
    };

    for category in &["allow", "ask", "deny"] {
        if let Some(items) = obj.get(*category)
            && let Some(arr) = items.as_array()
        {
            if arr.is_empty() {
                continue;
            }
            lines.push(format!("  Permissions ({category}):"));
            for item in arr {
                lines.push(format!("    {}", display_scalar(item)));
            }
        }
    }

    // Other permission keys
    for (key, val) in obj {
        if !["allow", "ask", "deny"].contains(&key.as_str()) {
            lines.push(format!("  Permissions ({key}): {}", format_inline(val)));
        }
    }
}

fn format_mcp_servers(val: &serde_json::Value, lines: &mut Vec<String>) {
    let obj = match val.as_object() {
        Some(o) => o,
        None => {
            lines.push(format!("  MCP Servers: {}", format_inline(val)));
            return;
        }
    };

    lines.push("  MCP Servers:".to_string());
    for (name, config) in obj {
        if let Some(cmd) = config.get("command") {
            let args = config
                .get("args")
                .and_then(|a| a.as_array())
                .map(|arr| arr.iter().map(display_scalar).collect::<Vec<_>>().join(" "))
                .unwrap_or_default();
            if args.is_empty() {
                lines.push(format!("    {name}: {}", display_scalar(cmd)));
            } else {
                lines.push(format!("    {name}: {} {args}", display_scalar(cmd)));
            }
        } else {
            lines.push(format!("    {name}: {}", format_inline(config)));
        }
    }
}

fn format_hooks(val: &serde_json::Value, lines: &mut Vec<String>) {
    let obj = match val.as_object() {
        Some(o) => o,
        None => {
            lines.push(format!("  Hooks: {}", format_inline(val)));
            return;
        }
    };

    lines.push("  Hooks:".to_string());
    for (event, hook_config) in obj {
        if let Some(arr) = hook_config.as_array() {
            for hook in arr {
                let cmd = hook
                    .get("command")
                    .map(display_scalar)
                    .unwrap_or_else(|| format_inline(hook));
                lines.push(format!("    {event}: {cmd}"));
            }
        } else {
            lines.push(format!("    {event}: {}", format_inline(hook_config)));
        }
    }
}

fn format_plugins(val: &serde_json::Value, lines: &mut Vec<String>) {
    let arr = match val.as_array() {
        Some(a) => a,
        None => {
            lines.push(format!("  Plugins: {}", format_inline(val)));
            return;
        }
    };

    lines.push("  Plugins:".to_string());
    for plugin in arr {
        lines.push(format!("    {}", display_scalar(plugin)));
    }
}

fn format_env(val: &serde_json::Value, lines: &mut Vec<String>) {
    let obj = match val.as_object() {
        Some(o) => o,
        None => {
            lines.push(format!("  Env: {}", format_inline(val)));
            return;
        }
    };

    lines.push("  Env:".to_string());
    for (key, val) in obj {
        lines.push(format!("    {key}={}", display_scalar(val)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_json(dir: &Path, rel_path: &str, content: &str) {
        let path = dir.join(rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn discovers_global_settings_when_present() {
        let home = TempDir::new().unwrap();
        let project = TempDir::new().unwrap();

        write_json(home.path(), ".claude/settings.json", r#"{"model":"opus"}"#);

        let collection = discover_settings_files_in(Some(home.path()), project.path());

        assert_eq!(collection.files.len(), 1);
        assert_eq!(collection.files[0].label, "Global");
    }

    #[test]
    fn discovers_project_settings() {
        let project = TempDir::new().unwrap();

        write_json(
            project.path(),
            ".claude/settings.json",
            r#"{"defaultMode":"plan"}"#,
        );

        let collection = discover_settings_files_in(None, project.path());

        assert_eq!(collection.files.len(), 1);
        assert_eq!(collection.files[0].label, "Project");
    }

    #[test]
    fn missing_files_skipped() {
        let project = TempDir::new().unwrap();
        // No settings files created
        let collection = discover_settings_files_in(None, project.path());
        assert!(collection.files.is_empty());
    }

    #[test]
    fn invalid_json_handled_gracefully() {
        let project = TempDir::new().unwrap();
        write_json(
            project.path(),
            ".claude/settings.json",
            "not valid json {{{",
        );

        let collection = discover_settings_files_in(None, project.path());

        assert_eq!(collection.files.len(), 1);
        let formatted = format_settings(&collection);
        assert!(
            formatted.iter().any(|l| l.contains("invalid JSON")),
            "Should show invalid JSON message, got: {:?}",
            formatted
        );
    }

    #[test]
    fn format_model_field() {
        let collection = collection_from_json(r#"{"model":"opus[1m]"}"#);
        let lines = format_settings(&collection);
        assert!(
            lines.iter().any(|l| l.contains("Model: opus[1m]")),
            "Expected model line, got: {:?}",
            lines
        );
    }

    #[test]
    fn format_permissions_section() {
        let collection =
            collection_from_json(r#"{"permissions":{"allow":["Read","Write"],"deny":["Bash"]}}"#);
        let lines = format_settings(&collection);
        assert!(lines.iter().any(|l| l.contains("Permissions (allow):")));
        assert!(lines.iter().any(|l| l.trim() == "Read"));
        assert!(lines.iter().any(|l| l.trim() == "Write"));
        assert!(lines.iter().any(|l| l.contains("Permissions (deny):")));
        assert!(lines.iter().any(|l| l.trim() == "Bash"));
    }

    #[test]
    fn format_mcp_servers() {
        let collection = collection_from_json(
            r#"{"mcpServers":{"filesystem":{"command":"npx","args":["-y","@modelcontextprotocol/server-filesystem"]}}}"#,
        );
        let lines = format_settings(&collection);
        assert!(lines.iter().any(|l| l.contains("MCP Servers:")));
        assert!(
            lines
                .iter()
                .any(|l| l.contains("filesystem:") && l.contains("npx")),
            "Expected filesystem server line, got: {:?}",
            lines
        );
    }

    #[test]
    fn format_hooks() {
        let collection =
            collection_from_json(r#"{"hooks":{"preCommit":[{"command":"cargo fmt"}]}}"#);
        let lines = format_settings(&collection);
        assert!(lines.iter().any(|l| l.contains("Hooks:")));
        assert!(
            lines
                .iter()
                .any(|l| l.contains("preCommit:") && l.contains("cargo fmt")),
            "Expected hook line, got: {:?}",
            lines
        );
    }

    #[test]
    fn format_plugins() {
        let collection = collection_from_json(r#"{"plugins":["plugin-a","plugin-b"]}"#);
        let lines = format_settings(&collection);
        assert!(lines.iter().any(|l| l.contains("Plugins:")));
        assert!(lines.iter().any(|l| l.trim() == "plugin-a"));
        assert!(lines.iter().any(|l| l.trim() == "plugin-b"));
    }

    #[test]
    fn format_env() {
        let collection = collection_from_json(r#"{"env":{"RUST_LOG":"debug","FOO":"bar"}}"#);
        let lines = format_settings(&collection);
        assert!(lines.iter().any(|l| l.contains("Env:")));
        assert!(lines.iter().any(|l| l.contains("RUST_LOG=debug")));
        assert!(lines.iter().any(|l| l.contains("FOO=bar")));
    }

    #[test]
    fn format_multiple_files_with_separators() {
        let collection = SettingsCollection {
            files: vec![
                SettingsFile {
                    label: "Global".to_string(),
                    path: PathBuf::from("/home/.claude/settings.json"),
                    value: serde_json::json!({"model": "opus"}),
                },
                SettingsFile {
                    label: "Project".to_string(),
                    path: PathBuf::from("/project/.claude/settings.json"),
                    value: serde_json::json!({"defaultMode": "plan"}),
                },
            ],
        };

        let lines = format_settings(&collection);

        // Should have a blank line between the two files
        let header_positions: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.starts_with('▾'))
            .map(|(i, _)| i)
            .collect();

        assert_eq!(header_positions.len(), 2, "Should have two section headers");
        assert!(
            header_positions[1] > header_positions[0] + 1,
            "Second header should come after first header's content"
        );

        // Blank line before second section
        let blank_before_second = &lines[header_positions[1] - 1];
        assert!(
            blank_before_second.is_empty(),
            "Should have blank separator line"
        );
    }

    /// Helper: create a SettingsCollection from a single JSON string.
    fn collection_from_json(json: &str) -> SettingsCollection {
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        SettingsCollection {
            files: vec![SettingsFile {
                label: "Test".to_string(),
                path: PathBuf::from("/test/settings.json"),
                value,
            }],
        }
    }
}
