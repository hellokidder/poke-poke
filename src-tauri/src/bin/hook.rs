//! poke-hook — Universal hook handler for Poke Poke.
//!
//! Supports Claude Code, Codex CLI, and Cursor.
//!
//! Two modes:
//!   1. Hook mode (no args): reads JSON from stdin, auto-detects source, handles events
//!   2. CLI mode: manages integration for each tool
//!      CC:     --install / --uninstall / --check
//!      Codex:  --install-codex / --uninstall-codex / --check-codex
//!      Cursor: --install-cursor <path> / --uninstall-cursor <path> / --check-cursor <path>

use serde_json::Value;
use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

const POKE_PORTS: &[u16] = &[9876, 9877];
const LOCK_DIR: &str = "/tmp";

// Events we register for Claude Code
const CC_HOOK_EVENTS: &[&str] = &["SessionStart", "UserPromptSubmit", "Notification", "Stop"];
// Events we register for Codex CLI
const CODEX_HOOK_EVENTS: &[&str] = &["SessionStart", "UserPromptSubmit", "Stop"];
// Events we register for Cursor
const CURSOR_HOOK_EVENTS: &[&str] = &["sessionStart", "beforeSubmitPrompt", "stop", "sessionEnd"];

// ==================== Source detection ====================

#[derive(Debug, Clone, Copy, PartialEq)]
enum Source {
    ClaudeCode,
    Codex,
    Cursor,
}

impl Source {
    fn as_str(&self) -> &'static str {
        match self {
            Source::ClaudeCode => "claude-code",
            Source::Codex => "codex",
            Source::Cursor => "cursor",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Source::ClaudeCode => "Claude Code",
            Source::Codex => "Codex",
            Source::Cursor => "Cursor",
        }
    }

    fn task_id_prefix(&self) -> &'static str {
        match self {
            Source::ClaudeCode => "cc",
            Source::Codex => "codex",
            Source::Cursor => "cursor",
        }
    }
}

fn detect_source(data: &Value) -> Source {
    if data.get("workspace_roots").is_some() || data.get("workspaceRoots").is_some() {
        Source::Cursor
    } else if data.get("turn_id").is_some()
        || data.get("stop_hook_active").is_some()
        || data.get("model").is_some()
        || matches!(
            data.get("source").and_then(|v| v.as_str()),
            Some("startup" | "resume" | "clear")
        )
    {
        Source::Codex
    } else {
        Source::ClaudeCode
    }
}

// ==================== Entry point ====================

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        // Claude Code (backward-compatible)
        Some("--install") => cmd_install_cc(),
        Some("--uninstall") => cmd_uninstall_cc(),
        Some("--check") => cmd_check_cc(),
        // Codex CLI
        Some("--install-codex") => cmd_install_codex(),
        Some("--uninstall-codex") => cmd_uninstall_codex(),
        Some("--check-codex") => cmd_check_codex(),
        // Cursor (project-local)
        Some("--install-cursor") => cmd_install_cursor(args.get(2)),
        Some("--uninstall-cursor") => cmd_uninstall_cursor(args.get(2)),
        Some("--check-cursor") => cmd_check_cursor(args.get(2)),
        // Hook mode
        _ => hook_mode(),
    }
}

// ==================== Shared helpers ====================

fn home_dir() -> PathBuf {
    PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
}

fn hook_bin_path() -> PathBuf {
    home_dir().join(".local/bin/poke-hook")
}

fn claude_settings_path() -> PathBuf {
    home_dir().join(".claude/settings.json")
}

fn codex_dir() -> PathBuf {
    home_dir().join(".codex")
}

fn print_json(val: &Value) {
    println!("{}", serde_json::to_string(val).unwrap_or_else(|_| "{}".into()));
}

/// Copy current binary to ~/.local/bin/poke-hook if needed.
fn ensure_binary_installed() -> bool {
    let target = hook_bin_path();
    let self_exe = env::current_exe().unwrap_or_default();

    if let Some(parent) = target.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if self_exe != target {
        if let Err(e) = fs::copy(&self_exe, &target) {
            print_json(&serde_json::json!({"status":"error","message":format!("Failed to copy binary: {}", e)}));
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&target, fs::Permissions::from_mode(0o755));
        }
    }
    true
}

/// Check if a hook group JSON contains a poke-hook command (CC/Codex format).
fn contains_poke_hook(group: &Value) -> bool {
    group["hooks"]
        .as_array()
        .map_or(false, |hooks| {
            hooks.iter().any(|h| {
                h["command"]
                    .as_str()
                    .map_or(false, |c| c.contains("poke-hook"))
            })
        })
}

/// Check if a Cursor hook entry contains poke-hook.
fn cursor_entry_has_poke_hook(entry: &Value) -> bool {
    entry["command"]
        .as_str()
        .map_or(false, |c| c.contains("poke-hook"))
}

/// Pick the first non-empty string value from multiple possible keys.
fn pick_str(data: &Value, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(s) = data.get(*k).and_then(|v| v.as_str()) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

// ==================== CC CLI subcommands ====================

fn cmd_install_cc() {
    if !ensure_binary_installed() {
        return;
    }

    // Read or create settings.json
    let settings_path = claude_settings_path();
    if let Some(parent) = settings_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut settings: Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path).unwrap_or_else(|_| "{}".into());
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Merge hooks
    let hooks = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let hook_command = hook_bin_path().display().to_string();
    let new_entry = serde_json::json!([{
        "hooks": [{"type": "command", "command": hook_command}]
    }]);

    for event in CC_HOOK_EVENTS {
        let event_hooks = hooks
            .as_object_mut()
            .unwrap()
            .entry(*event)
            .or_insert_with(|| serde_json::json!([]));

        if let Some(arr) = event_hooks.as_array_mut() {
            arr.retain(|group| !contains_poke_hook(group));
        }
        if let Some(arr) = event_hooks.as_array_mut() {
            if let Some(new_arr) = new_entry.as_array() {
                arr.extend(new_arr.iter().cloned());
            }
        }
    }

    let pretty = serde_json::to_string_pretty(&settings).unwrap_or_default();
    if let Err(e) = fs::write(&settings_path, &pretty) {
        print_json(&serde_json::json!({"status":"error","message":format!("Failed to write settings: {}", e)}));
        return;
    }

    print_json(&serde_json::json!({
        "status": "ok",
        "message": format!("Installed poke-hook to {} and configured {} hooks", hook_command, CC_HOOK_EVENTS.len())
    }));
}

fn cmd_uninstall_cc() {
    let settings_path = claude_settings_path();
    if !settings_path.exists() {
        print_json(&serde_json::json!({"status":"ok","message":"No settings file found, nothing to do"}));
        return;
    }

    let content = fs::read_to_string(&settings_path).unwrap_or_else(|_| "{}".into());
    let mut settings: Value = serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}));

    const LEGACY_EVENTS: &[&str] = &["PreToolUse", "PostToolUse"];

    if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for event in CC_HOOK_EVENTS.iter().chain(LEGACY_EVENTS.iter()) {
            if let Some(event_hooks) = hooks.get_mut(*event).and_then(|v| v.as_array_mut()) {
                event_hooks.retain(|group| !contains_poke_hook(group));
            }
        }
        let empty_keys: Vec<String> = hooks
            .iter()
            .filter(|(_, v)| v.as_array().map_or(false, |a| a.is_empty()))
            .map(|(k, _)| k.clone())
            .collect();
        for key in empty_keys {
            hooks.remove(&key);
        }
        if hooks.is_empty() {
            settings.as_object_mut().unwrap().remove("hooks");
        }
    }

    let pretty = serde_json::to_string_pretty(&settings).unwrap_or_default();
    let _ = fs::write(&settings_path, &pretty);

    print_json(&serde_json::json!({"status":"ok","message":"Removed poke-hook from Claude Code settings"}));
}

fn cmd_check_cc() {
    let installed = hook_bin_path().exists();

    let hooks_configured = if claude_settings_path().exists() {
        let content = fs::read_to_string(claude_settings_path()).unwrap_or_default();
        let settings: Value = serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
        CC_HOOK_EVENTS.iter().all(|event| {
            settings["hooks"][event]
                .as_array()
                .map_or(false, |arr| arr.iter().any(|g| contains_poke_hook(g)))
        })
    } else {
        false
    };

    print_json(&serde_json::json!({
        "installed": installed,
        "hooks_configured": hooks_configured,
        "connected": installed && hooks_configured
    }));
}

// ==================== Codex CLI subcommands ====================

fn cmd_install_codex() {
    if !ensure_binary_installed() {
        return;
    }

    let dir = codex_dir();
    let _ = fs::create_dir_all(&dir);

    // 1. Write hooks.json
    let hooks_path = dir.join("hooks.json");
    let hook_command = hook_bin_path().display().to_string();

    let mut hooks_map = serde_json::Map::new();
    for event in CODEX_HOOK_EVENTS {
        hooks_map.insert(
            event.to_string(),
            serde_json::json!([{
                "hooks": [{"type": "command", "command": &hook_command}]
            }]),
        );
    }
    let hooks_json = serde_json::json!({ "hooks": hooks_map });
    if let Err(e) = fs::write(&hooks_path, serde_json::to_string_pretty(&hooks_json).unwrap_or_default()) {
        print_json(&serde_json::json!({"status":"error","message":format!("Failed to write hooks.json: {}", e)}));
        return;
    }

    // 2. Edit config.toml: add hooks path + enable feature flag
    let config_path = dir.join("config.toml");
    let content = fs::read_to_string(&config_path).unwrap_or_default();
    let mut doc = content
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_else(|_| toml_edit::DocumentMut::new());

    doc["hooks"] = toml_edit::value("./hooks.json");

    if doc.get("features").is_none() {
        doc["features"] = toml_edit::Item::Table(toml_edit::Table::new());
    }
    doc["features"]["codex_hooks"] = toml_edit::value(true);

    if let Err(e) = fs::write(&config_path, doc.to_string()) {
        print_json(&serde_json::json!({"status":"error","message":format!("Failed to write config.toml: {}", e)}));
        return;
    }

    print_json(&serde_json::json!({
        "status": "ok",
        "message": "Installed poke-hook for Codex CLI: hooks.json created + config.toml updated"
    }));
}

fn cmd_uninstall_codex() {
    let dir = codex_dir();

    // Clean hooks.json
    let hooks_path = dir.join("hooks.json");
    if hooks_path.exists() {
        let content = fs::read_to_string(&hooks_path).unwrap_or_default();
        if let Ok(mut val) = serde_json::from_str::<Value>(&content) {
            if let Some(hooks) = val.get_mut("hooks").and_then(|h| h.as_object_mut()) {
                for (_, event_hooks) in hooks.iter_mut() {
                    if let Some(arr) = event_hooks.as_array_mut() {
                        arr.retain(|group| !contains_poke_hook(group));
                    }
                }
                hooks.retain(|_, v| v.as_array().map_or(true, |a| !a.is_empty()));
            }
            if val["hooks"].as_object().map_or(true, |h| h.is_empty()) {
                let _ = fs::remove_file(&hooks_path);
            } else {
                let _ = fs::write(&hooks_path, serde_json::to_string_pretty(&val).unwrap_or_default());
            }
        }
    }

    // Remove hooks key from config.toml
    let config_path = dir.join("config.toml");
    if config_path.exists() {
        let content = fs::read_to_string(&config_path).unwrap_or_default();
        if let Ok(mut doc) = content.parse::<toml_edit::DocumentMut>() {
            doc.remove("hooks");
            let _ = fs::write(&config_path, doc.to_string());
        }
    }

    print_json(&serde_json::json!({"status":"ok","message":"Removed poke-hook from Codex CLI config"}));
}

fn cmd_check_codex() {
    let installed = hook_bin_path().exists();
    let dir = codex_dir();

    let hooks_configured = {
        let hooks_path = dir.join("hooks.json");
        if hooks_path.exists() {
            let content = fs::read_to_string(&hooks_path).unwrap_or_default();
            let val: Value = serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
            val["hooks"]["Stop"]
                .as_array()
                .map_or(false, |arr| arr.iter().any(|g| contains_poke_hook(g)))
        } else {
            false
        }
    };

    let feature_enabled = {
        let config_path = dir.join("config.toml");
        if config_path.exists() {
            let content = fs::read_to_string(&config_path).unwrap_or_default();
            content
                .parse::<toml_edit::DocumentMut>()
                .ok()
                .and_then(|doc| {
                    doc.get("features")
                        .and_then(|f| f.get("codex_hooks"))
                        .and_then(|v| v.as_bool())
                })
                .unwrap_or(false)
        } else {
            false
        }
    };

    print_json(&serde_json::json!({
        "installed": installed,
        "hooks_configured": hooks_configured,
        "feature_enabled": feature_enabled,
        "connected": installed && hooks_configured && feature_enabled
    }));
}

// ==================== Cursor CLI subcommands ====================

fn cmd_install_cursor(project_path: Option<&String>) {
    let project = match project_path {
        Some(p) => PathBuf::from(p),
        None => {
            print_json(&serde_json::json!({"status":"error","message":"Usage: poke-hook --install-cursor <project-path>"}));
            return;
        }
    };

    if !ensure_binary_installed() {
        return;
    }

    let cursor_dir = project.join(".cursor");
    let _ = fs::create_dir_all(&cursor_dir);

    let hooks_path = cursor_dir.join("hooks.json");
    let hook_command = hook_bin_path().display().to_string();

    // Read existing or create new
    let mut hooks: Value = if hooks_path.exists() {
        let content = fs::read_to_string(&hooks_path).unwrap_or_else(|_| "{}".into());
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    hooks["version"] = serde_json::json!(1);
    let hooks_obj = hooks
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let new_entry = serde_json::json!({"command": &hook_command, "timeout": 8});

    for event in CURSOR_HOOK_EVENTS {
        let event_hooks = hooks_obj
            .as_object_mut()
            .unwrap()
            .entry(*event)
            .or_insert_with(|| serde_json::json!([]));

        if let Some(arr) = event_hooks.as_array_mut() {
            arr.retain(|entry| !cursor_entry_has_poke_hook(entry));
            arr.push(new_entry.clone());
        }
    }

    let pretty = serde_json::to_string_pretty(&hooks).unwrap_or_default();
    if let Err(e) = fs::write(&hooks_path, &pretty) {
        print_json(&serde_json::json!({"status":"error","message":format!("Failed to write hooks.json: {}", e)}));
        return;
    }

    // Clean up legacy Python/bash scripts if present
    let _ = fs::remove_file(cursor_dir.join("hooks/pokepokeCursorStop.py"));
    let _ = fs::remove_file(cursor_dir.join("hooks/runPokepokeNotify.sh"));
    // Remove hooks dir if empty
    let _ = fs::remove_dir(cursor_dir.join("hooks"));

    print_json(&serde_json::json!({
        "status": "ok",
        "message": format!("Installed Cursor hooks at {}", hooks_path.display())
    }));
}

fn cmd_uninstall_cursor(project_path: Option<&String>) {
    let project = match project_path {
        Some(p) => PathBuf::from(p),
        None => {
            print_json(&serde_json::json!({"status":"error","message":"Usage: poke-hook --uninstall-cursor <project-path>"}));
            return;
        }
    };

    let hooks_path = project.join(".cursor/hooks.json");
    if !hooks_path.exists() {
        print_json(&serde_json::json!({"status":"ok","message":"No .cursor/hooks.json found"}));
        return;
    }

    let content = fs::read_to_string(&hooks_path).unwrap_or_else(|_| "{}".into());
    let mut hooks: Value = serde_json::from_str(&content).unwrap_or(serde_json::json!({}));

    if let Some(hooks_obj) = hooks.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for (_, event_hooks) in hooks_obj.iter_mut() {
            if let Some(arr) = event_hooks.as_array_mut() {
                arr.retain(|entry| !cursor_entry_has_poke_hook(entry));
            }
        }
        hooks_obj.retain(|_, v| v.as_array().map_or(true, |a| !a.is_empty()));
    }

    if hooks["hooks"].as_object().map_or(true, |h| h.is_empty()) {
        let _ = fs::remove_file(&hooks_path);
    } else {
        let _ = fs::write(&hooks_path, serde_json::to_string_pretty(&hooks).unwrap_or_default());
    }

    print_json(&serde_json::json!({"status":"ok","message":"Removed poke-hook from Cursor hooks"}));
}

fn cmd_check_cursor(project_path: Option<&String>) {
    let project = match project_path {
        Some(p) => PathBuf::from(p),
        None => {
            print_json(&serde_json::json!({"installed": false, "hooks_configured": false, "connected": false}));
            return;
        }
    };

    let installed = hook_bin_path().exists();

    let hooks_path = project.join(".cursor/hooks.json");
    let hooks_configured = if hooks_path.exists() {
        let content = fs::read_to_string(&hooks_path).unwrap_or_default();
        let val: Value = serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
        // Check if at least "stop" event has poke-hook
        val["hooks"]["stop"]
            .as_array()
            .map_or(false, |arr| arr.iter().any(|e| cursor_entry_has_poke_hook(e)))
    } else {
        false
    };

    print_json(&serde_json::json!({
        "installed": installed,
        "hooks_configured": hooks_configured,
        "connected": installed && hooks_configured
    }));
}

// ==================== Hook mode ====================

fn hook_mode() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap_or_default();

    // Always output valid JSON for the caller
    let _guard = PrintOnDrop;

    let data: Value = match serde_json::from_str(input.trim()) {
        Ok(v) => v,
        Err(_) => return,
    };

    let source = detect_source(&data);

    // Extract event name (CC/Codex: hook_event_name, Cursor: hookEventName or hook_event_name)
    let raw_event = pick_str(&data, &["hook_event_name", "hookEventName"]).unwrap_or_default();

    // Normalize Cursor camelCase events to internal names
    let event = normalize_event(&raw_event);

    // Build stable task_id
    let session_key = match source {
        Source::Cursor => {
            pick_str(&data, &["conversation_id", "conversationId"])
                .or_else(|| pick_str(&data, &["generation_id", "generationId"]))
                .unwrap_or_else(|| "unknown".into())
        }
        _ => data["session_id"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
    };
    let short_key: String = session_key.chars().take(8).collect();
    let task_id = format!("{}-{}", source.task_id_prefix(), short_key);

    // Extract workspace path
    let cwd = match source {
        Source::Cursor => {
            let roots = data
                .get("workspace_roots")
                .or_else(|| data.get("workspaceRoots"));
            roots
                .and_then(|r| r.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_default()
        }
        _ => data["cwd"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| env::current_dir().unwrap_or_default().display().to_string()),
    };

    let project = PathBuf::from(&cwd)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    match event.as_str() {
        "SessionStart" => handle_session_start(&task_id, &project, &cwd, source),
        "UserPromptSubmit" => handle_user_prompt_submit(&task_id, &project, &cwd, source),
        "Notification" => {
            // CC only — Codex/Cursor don't have this event
            if source == Source::ClaudeCode {
                let message = data["message"].as_str().unwrap_or("");
                handle_notification(&task_id, &project, message, &cwd);
            }
        }
        "Stop" => {
            if source == Source::Cursor {
                let hook_status =
                    pick_str(&data, &["status", "hookStatus"]).unwrap_or_else(|| "completed".into());
                handle_cursor_stop(&task_id, &project, &cwd, &hook_status);
            } else {
                handle_stop(&task_id, &project, &cwd, source);
            }
        }
        "SessionEnd" => {
            // Cursor only
            handle_session_end(&task_id, &project, &cwd);
        }
        _ => {}
    }
}

/// Normalize Cursor camelCase event names to internal PascalCase.
fn normalize_event(raw: &str) -> String {
    match raw {
        "sessionStart" => "SessionStart".into(),
        "beforeSubmitPrompt" => "UserPromptSubmit".into(),
        "stop" => "Stop".into(),
        "sessionEnd" => "SessionEnd".into(),
        other => other.into(),
    }
}

// ==================== Event handlers ====================

fn handle_session_start(task_id: &str, project: &str, cwd: &str, source: Source) {
    let tty = get_tty();
    let mut payload = serde_json::json!({
        "task_id": task_id,
        "title": format!("{}: {}", source.label(), project),
        "message": format!("Session started\n{}", cwd),
        "source": source.as_str(),
        "status": "running",
        "workspace_path": cwd,
    });
    if let Some(ref t) = tty {
        payload["terminal_tty"] = Value::String(t.clone());
    }
    post_notify(&payload);
}

fn handle_user_prompt_submit(task_id: &str, project: &str, cwd: &str, source: Source) {
    let lock_file = flag_path(task_id, "registered");

    // Clear pending flag if set
    let pending_flag = flag_path(task_id, "pending");
    if pending_flag.exists() {
        let _ = fs::remove_file(&pending_flag);
    }

    if lock_file.exists() {
        let payload = serde_json::json!({
            "task_id": task_id,
            "title": format!("{}: {}", source.label(), project),
            "message": format!("Working...\n{}", cwd),
            "source": source.as_str(),
            "status": "running",
        });
        post_notify(&payload);
        return;
    }

    let _ = fs::write(&lock_file, std::process::id().to_string());

    let tty = get_tty();
    let mut payload = serde_json::json!({
        "task_id": task_id,
        "title": format!("{}: {}", source.label(), project),
        "message": format!("Working...\n{}", cwd),
        "source": source.as_str(),
        "status": "running",
        "workspace_path": cwd,
    });
    if let Some(ref t) = tty {
        payload["terminal_tty"] = Value::String(t.clone());
    }
    post_notify(&payload);
}

fn handle_notification(task_id: &str, project: &str, message: &str, cwd: &str) {
    let flag = flag_path(task_id, "pending");
    let _ = fs::write(&flag, message);

    let tty = get_tty();
    let mut payload = serde_json::json!({
        "task_id": task_id,
        "title": format!("Claude Code: {}", project),
        "message": message,
        "source": "claude-code",
        "status": "pending",
        "workspace_path": cwd,
    });
    if let Some(ref t) = tty {
        payload["terminal_tty"] = Value::String(t.clone());
    }
    post_notify(&payload);
}

fn handle_stop(task_id: &str, project: &str, cwd: &str, source: Source) {
    let _ = fs::remove_file(flag_path(task_id, "registered"));
    let _ = fs::remove_file(flag_path(task_id, "pending"));

    let tty = get_tty();
    let mut payload = serde_json::json!({
        "task_id": task_id,
        "title": format!("{}: {}", source.label(), project),
        "message": "Session completed",
        "source": source.as_str(),
        "status": "success",
        "workspace_path": cwd,
    });
    if let Some(ref t) = tty {
        payload["terminal_tty"] = Value::String(t.clone());
    }
    post_notify(&payload);
}

fn handle_cursor_stop(task_id: &str, project: &str, cwd: &str, hook_status: &str) {
    let _ = fs::remove_file(flag_path(task_id, "registered"));
    let _ = fs::remove_file(flag_path(task_id, "pending"));

    let (status, msg) = match hook_status {
        "completed" => ("success", "Agent turn completed"),
        "aborted" => ("success", "Agent turn aborted"),
        _ => ("success", "Agent turn ended with error"),
    };

    let payload = serde_json::json!({
        "task_id": task_id,
        "title": format!("Cursor: {}", project),
        "message": msg,
        "source": "cursor",
        "status": status,
        "workspace_path": cwd,
    });
    post_notify(&payload);
}

fn handle_session_end(task_id: &str, project: &str, cwd: &str) {
    let _ = fs::remove_file(flag_path(task_id, "registered"));
    let _ = fs::remove_file(flag_path(task_id, "pending"));

    let payload = serde_json::json!({
        "task_id": task_id,
        "title": format!("Cursor: {}", project),
        "message": "Session ended",
        "source": "cursor",
        "status": "success",
        "workspace_path": cwd,
    });
    post_notify(&payload);
}

// ==================== Low-level helpers ====================

fn flag_path(task_id: &str, suffix: &str) -> PathBuf {
    PathBuf::from(LOCK_DIR).join(format!("pokepoke-{}.{}", task_id, suffix))
}

fn post_notify(payload: &Value) -> bool {
    let body = payload.to_string();
    for port in POKE_PORTS {
        let url = format!("http://127.0.0.1:{}/notify", port);
        let result = ureq::post(&url)
            .set("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(2))
            .send_string(&body);
        if result.is_ok() {
            return true;
        }
    }
    false
}

fn get_tty() -> Option<String> {
    let mut pid = std::process::id();
    for _ in 0..8 {
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "tty=,ppid="])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = text.trim().split_whitespace().collect();
        if parts.is_empty() {
            break;
        }
        let tty_name = parts[0];
        let ppid: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

        if !tty_name.is_empty() && tty_name != "??" {
            return Some(format!("/dev/{}", tty_name));
        }
        if ppid <= 1 {
            break;
        }
        pid = ppid;
    }
    None
}

/// Prints `{}` to stdout when dropped (even on early return).
struct PrintOnDrop;
impl Drop for PrintOnDrop {
    fn drop(&mut self) {
        println!("{{}}");
    }
}
