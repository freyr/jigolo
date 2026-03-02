use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SettingsFile {
    pub label: String,
    pub path: PathBuf,
    pub value: serde_json::Value,
}

#[derive(Debug, Default, Clone)]
pub struct SettingsCollection {
    pub files: Vec<SettingsFile>,
}

/// Settings keys rendered in this order. Shared between per-file and merged formatters.
const ORDERED_SETTINGS_KEYS: &[&str] = &[
    "model",
    "defaultMode",
    "thinking",
    "permissions",
    "mcpServers",
    "hooks",
    "plugins",
    "env",
];

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

/// Maps each display line index to the index of the source `SettingsFile`.
/// Blank separator lines map to `None`.
pub type SettingsLineMap = Vec<Option<usize>>;

/// Format settings collection into display lines and a line-to-file mapping.
pub fn format_settings_with_map(collection: &SettingsCollection) -> (Vec<String>, SettingsLineMap) {
    let mut lines = Vec::new();
    let mut line_map = Vec::new();

    for (i, file) in collection.files.iter().enumerate() {
        if i > 0 {
            lines.push(String::new());
            line_map.push(None); // blank separator
        }

        // Section header
        lines.push(format!("▾ {} ({})", file.label, file.path.display()));
        line_map.push(Some(i));

        // If the value is an error string, just show it
        if let serde_json::Value::String(s) = &file.value {
            lines.push(format!("  {s}"));
            line_map.push(Some(i));
            continue;
        }

        let obj = match file.value.as_object() {
            Some(obj) => obj,
            None => {
                lines.push("  (not a JSON object)".to_string());
                line_map.push(Some(i));
                continue;
            }
        };

        let before = lines.len();
        for &key in ORDERED_SETTINGS_KEYS {
            if let Some(val) = obj.get(key) {
                format_key_value(key, val, &mut lines);
            }
        }
        for (key, val) in obj {
            if !ORDERED_SETTINGS_KEYS.contains(&key.as_str()) {
                format_key_value(key, val, &mut lines);
            }
        }
        let added = lines.len() - before;
        for _ in 0..added {
            line_map.push(Some(i));
        }
    }

    (lines, line_map)
}

/// Format settings collection into display lines for the TUI.
pub fn format_settings(collection: &SettingsCollection) -> Vec<String> {
    let (lines, _) = format_settings_with_map(collection);
    lines
}

/// Merges settings files into a single effective JSON value.
///
/// Scalars use last-writer-wins. Array fields (permissions sub-keys, plugins)
/// use set union with deduplication. Hooks are concatenated per event key.
/// Objects (mcpServers, env) merge by key with later files winning.
pub fn merge_settings(collection: &SettingsCollection) -> serde_json::Value {
    let mut result = serde_json::Map::new();

    for file in &collection.files {
        let Some(obj) = file.value.as_object() else {
            continue;
        };

        for (key, val) in obj {
            match key.as_str() {
                "permissions" => merge_permissions(&mut result, val),
                "hooks" => merge_hooks(&mut result, val),
                "plugins" => merge_array_union(&mut result, "plugins", val),
                _ => {
                    // mcpServers, env: merge objects by key; scalars: replace
                    if val.is_object() {
                        merge_object(&mut result, key, val);
                    } else {
                        result.insert(key.clone(), val.clone());
                    }
                }
            }
        }
    }

    serde_json::Value::Object(result)
}

fn merge_permissions(result: &mut serde_json::Map<String, serde_json::Value>, val: &serde_json::Value) {
    let Some(incoming) = val.as_object() else {
        return;
    };
    let existing = result
        .entry("permissions")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let Some(existing_obj) = existing.as_object_mut() else {
        return;
    };

    for (sub_key, sub_val) in incoming {
        merge_array_union(existing_obj, sub_key, sub_val);
    }
}

fn merge_hooks(result: &mut serde_json::Map<String, serde_json::Value>, val: &serde_json::Value) {
    let Some(incoming) = val.as_object() else {
        return;
    };
    let existing = result
        .entry("hooks")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let Some(existing_obj) = existing.as_object_mut() else {
        return;
    };

    for (event, hooks_val) in incoming {
        let Some(incoming_arr) = hooks_val.as_array() else {
            existing_obj.insert(event.clone(), hooks_val.clone());
            continue;
        };
        let entry = existing_obj
            .entry(event)
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        if let Some(arr) = entry.as_array_mut() {
            arr.extend(incoming_arr.iter().cloned());
        }
    }
}

fn merge_array_union(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    val: &serde_json::Value,
) {
    let Some(incoming_arr) = val.as_array() else {
        obj.insert(key.to_string(), val.clone());
        return;
    };
    let entry = obj
        .entry(key)
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    if let Some(existing_arr) = entry.as_array_mut() {
        for item in incoming_arr {
            if !existing_arr.contains(item) {
                existing_arr.push(item.clone());
            }
        }
    }
}

fn merge_object(
    result: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    val: &serde_json::Value,
) {
    let Some(incoming_obj) = val.as_object() else {
        result.insert(key.to_string(), val.clone());
        return;
    };
    let entry = result
        .entry(key)
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if let Some(existing_obj) = entry.as_object_mut() {
        for (k, v) in incoming_obj {
            existing_obj.insert(k.clone(), v.clone());
        }
    }
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

    #[test]
    fn line_map_maps_lines_to_correct_files() {
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

        let (lines, line_map) = format_settings_with_map(&collection);

        assert_eq!(lines.len(), line_map.len(), "Lines and map should match");

        // First file header should map to file index 0
        assert_eq!(line_map[0], Some(0));

        // Find the blank separator
        let blank_idx = lines.iter().position(|l| l.is_empty()).unwrap();
        assert_eq!(line_map[blank_idx], None, "Blank separator maps to None");

        // Line after separator should map to file index 1
        assert_eq!(line_map[blank_idx + 1], Some(1));
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

    fn two_file_collection(global_json: &str, project_json: &str) -> SettingsCollection {
        SettingsCollection {
            files: vec![
                SettingsFile {
                    label: "Global".to_string(),
                    path: PathBuf::from("/global/settings.json"),
                    value: serde_json::from_str(global_json).unwrap(),
                },
                SettingsFile {
                    label: "Project".to_string(),
                    path: PathBuf::from("/project/settings.json"),
                    value: serde_json::from_str(project_json).unwrap(),
                },
            ],
        }
    }

    #[test]
    fn merge_scalars_last_writer_wins() {
        let collection = two_file_collection(
            r#"{"model":"haiku","defaultMode":"plan"}"#,
            r#"{"model":"opus"}"#,
        );
        let merged = merge_settings(&collection);
        let obj = merged.as_object().unwrap();
        assert_eq!(obj.get("model").unwrap().as_str().unwrap(), "opus");
        assert_eq!(obj.get("defaultMode").unwrap().as_str().unwrap(), "plan");
    }

    #[test]
    fn merge_permissions_are_additive() {
        let collection = two_file_collection(
            r#"{"permissions":{"allow":["Read","Write"]}}"#,
            r#"{"permissions":{"allow":["Write","Bash"],"deny":["rm"]}}"#,
        );
        let merged = merge_settings(&collection);
        let perms = merged.get("permissions").unwrap().as_object().unwrap();
        let allow: Vec<&str> = perms
            .get("allow")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(allow, vec!["Read", "Write", "Bash"]);
        let deny: Vec<&str> = perms
            .get("deny")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(deny, vec!["rm"]);
    }

    #[test]
    fn merge_mcp_servers_by_key() {
        let collection = two_file_collection(
            r#"{"mcpServers":{"ctx7":{"command":"npx"}}}"#,
            r#"{"mcpServers":{"ctx7":{"command":"node"},"gh":{"command":"gh"}}}"#,
        );
        let merged = merge_settings(&collection);
        let servers = merged.get("mcpServers").unwrap().as_object().unwrap();
        assert_eq!(
            servers.get("ctx7").unwrap().get("command").unwrap().as_str().unwrap(),
            "node"
        );
        assert!(servers.contains_key("gh"));
    }

    #[test]
    fn merge_hooks_concatenated() {
        let collection = two_file_collection(
            r#"{"hooks":{"preCommit":[{"command":"fmt"}]}}"#,
            r#"{"hooks":{"preCommit":[{"command":"lint"}],"prePush":[{"command":"test"}]}}"#,
        );
        let merged = merge_settings(&collection);
        let hooks = merged.get("hooks").unwrap().as_object().unwrap();
        let pre_commit = hooks.get("preCommit").unwrap().as_array().unwrap();
        assert_eq!(pre_commit.len(), 2);
        assert!(hooks.contains_key("prePush"));
    }

    #[test]
    fn merge_env_last_writer_wins() {
        let collection = two_file_collection(
            r#"{"env":{"LOG":"info","HOME":"/a"}}"#,
            r#"{"env":{"LOG":"debug"}}"#,
        );
        let merged = merge_settings(&collection);
        let env = merged.get("env").unwrap().as_object().unwrap();
        assert_eq!(env.get("LOG").unwrap().as_str().unwrap(), "debug");
        assert_eq!(env.get("HOME").unwrap().as_str().unwrap(), "/a");
    }

    #[test]
    fn merge_plugins_deduplicated() {
        let collection = two_file_collection(
            r#"{"plugins":["a","b"]}"#,
            r#"{"plugins":["b","c"]}"#,
        );
        let merged = merge_settings(&collection);
        let plugins: Vec<&str> = merged
            .get("plugins")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(plugins, vec!["a", "b", "c"]);
    }

    #[test]
    fn merge_skips_invalid_json() {
        let collection = SettingsCollection {
            files: vec![
                SettingsFile {
                    label: "Global".to_string(),
                    path: PathBuf::from("/global"),
                    value: serde_json::from_str(r#"{"model":"opus"}"#).unwrap(),
                },
                SettingsFile {
                    label: "Project".to_string(),
                    path: PathBuf::from("/project"),
                    value: serde_json::Value::String("(invalid JSON)".to_string()),
                },
            ],
        };
        let merged = merge_settings(&collection);
        assert_eq!(
            merged.get("model").unwrap().as_str().unwrap(),
            "opus"
        );
    }
}
