//! poke-hook — Claude Code hook handler for Poke Poke.
//!
//! Two modes:
//!   1. Hook mode (no args): reads JSON from stdin, handles CC hook events
//!   2. CLI mode (--install / --uninstall / --check): manages CC integration

use serde_json::Value;
use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

const POKE_PORTS: &[u16] = &[9876, 9877];
const LOCK_DIR: &str = "/tmp";
const HOOK_EVENTS: &[&str] = &["SessionStart", "UserPromptSubmit", "Notification", "Stop"];

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("--install") => cmd_install(),
        Some("--uninstall") => cmd_uninstall(),
        Some("--check") => cmd_check(),
        _ => hook_mode(),
    }
}

// ==================== CLI subcommands ====================

fn home_dir() -> PathBuf {
    PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
}

fn hook_bin_path() -> PathBuf {
    home_dir().join(".local/bin/poke-hook")
}

fn claude_settings_path() -> PathBuf {
    home_dir().join(".claude/settings.json")
}

fn cmd_install() {
    // 1. Copy binary to ~/.local/bin/poke-hook
    let target = hook_bin_path();
    let self_exe = env::current_exe().unwrap_or_default();

    if let Some(parent) = target.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Copy if target doesn't exist or is a different file
    if self_exe != target {
        if let Err(e) = fs::copy(&self_exe, &target) {
            print_json(&serde_json::json!({"status":"error","message":format!("Failed to copy binary: {}", e)}));
            return;
        }
        // Ensure executable permission
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&target, fs::Permissions::from_mode(0o755));
        }
    }

    // 2. Read or create settings.json
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

    // 3. Merge hooks
    let hooks = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let hook_command = target.display().to_string();
    let new_entry = serde_json::json!([{
        "hooks": [{"type": "command", "command": hook_command}]
    }]);

    for event in HOOK_EVENTS {
        let event_hooks = hooks
            .as_object_mut()
            .unwrap()
            .entry(*event)
            .or_insert_with(|| serde_json::json!([]));

        // Remove existing poke-hook entries
        if let Some(arr) = event_hooks.as_array_mut() {
            arr.retain(|group| !contains_poke_hook(group));
        }

        // Append our hook
        if let Some(arr) = event_hooks.as_array_mut() {
            if let Some(new_arr) = new_entry.as_array() {
                arr.extend(new_arr.iter().cloned());
            }
        }
    }

    // 4. Write back
    let pretty = serde_json::to_string_pretty(&settings).unwrap_or_default();
    if let Err(e) = fs::write(&settings_path, &pretty) {
        print_json(&serde_json::json!({"status":"error","message":format!("Failed to write settings: {}", e)}));
        return;
    }

    print_json(&serde_json::json!({
        "status": "ok",
        "message": format!("Installed poke-hook to {} and configured {} hooks", hook_command, HOOK_EVENTS.len())
    }));
}

fn cmd_uninstall() {
    let settings_path = claude_settings_path();
    if !settings_path.exists() {
        print_json(&serde_json::json!({"status":"ok","message":"No settings file found, nothing to do"}));
        return;
    }

    let content = fs::read_to_string(&settings_path).unwrap_or_else(|_| "{}".into());
    let mut settings: Value = serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}));

    // Also clean up legacy event names no longer in HOOK_EVENTS
    const LEGACY_EVENTS: &[&str] = &["PreToolUse", "PostToolUse"];

    if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for event in HOOK_EVENTS.iter().chain(LEGACY_EVENTS.iter()) {
            if let Some(event_hooks) = hooks.get_mut(*event).and_then(|v| v.as_array_mut()) {
                event_hooks.retain(|group| !contains_poke_hook(group));
            }
        }
        // Clean up empty event keys
        let empty_keys: Vec<String> = hooks
            .iter()
            .filter(|(_, v)| v.as_array().map_or(false, |a| a.is_empty()))
            .map(|(k, _)| k.clone())
            .collect();
        for key in empty_keys {
            hooks.remove(&key);
        }
        // Remove hooks key entirely if empty
        if hooks.is_empty() {
            settings.as_object_mut().unwrap().remove("hooks");
        }
    }

    let pretty = serde_json::to_string_pretty(&settings).unwrap_or_default();
    let _ = fs::write(&settings_path, &pretty);

    print_json(&serde_json::json!({"status":"ok","message":"Removed poke-hook from Claude Code settings"}));
}

fn cmd_check() {
    let installed = hook_bin_path().exists();

    let hooks_configured = if claude_settings_path().exists() {
        let content = fs::read_to_string(claude_settings_path()).unwrap_or_default();
        let settings: Value = serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
        HOOK_EVENTS.iter().all(|event| {
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

/// Check if a hook group JSON contains a poke-hook command
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

fn print_json(val: &Value) {
    println!("{}", serde_json::to_string(val).unwrap_or_else(|_| "{}".into()));
}

// ==================== Hook mode ====================

fn hook_mode() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap_or_default();

    // Always output valid JSON for CC
    let _guard = PrintOnDrop;

    let data: Value = match serde_json::from_str(input.trim()) {
        Ok(v) => v,
        Err(_) => return,
    };

    let event = data["hook_event_name"].as_str().unwrap_or("");
    let session_id = &data["session_id"]
        .as_str()
        .unwrap_or("unknown")
        .chars()
        .take(8)
        .collect::<String>();
    let task_id = format!("cc-{}", session_id);
    let cwd = data["cwd"]
        .as_str()
        .map(String::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_default().display().to_string());
    let project = PathBuf::from(&cwd)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    match event {
        "SessionStart" => handle_session_start(&task_id, &project, &cwd),
        "Notification" => {
            let message = data["message"].as_str().unwrap_or("");
            handle_notification(&task_id, &project, message, &cwd);
        }
        "UserPromptSubmit" => handle_user_prompt_submit(&task_id, &project, &cwd),
        "Stop" => handle_stop(&task_id, &project, &cwd),
        _ => {}
    }
}

// ---------- event handlers ----------

fn handle_session_start(task_id: &str, project: &str, cwd: &str) {
    let tty = get_tty();
    let mut payload = serde_json::json!({
        "task_id": task_id,
        "title": format!("Claude Code: {}", project),
        "message": format!("Session started\n{}", cwd),
        "source": "claude-code",
        "status": "running",
        "workspace_path": cwd,
    });
    if let Some(ref t) = tty {
        payload["terminal_tty"] = Value::String(t.clone());
    }
    post_notify(&payload);
}

fn handle_user_prompt_submit(task_id: &str, project: &str, cwd: &str) {
    let lock_file = flag_path(task_id, "registered");

    // Already registered — just ensure running status (clears pending if needed)
    let pending_flag = flag_path(task_id, "pending");
    if pending_flag.exists() {
        let _ = fs::remove_file(&pending_flag);
    }

    if lock_file.exists() {
        // Still send running status to update the session
        let payload = serde_json::json!({
            "task_id": task_id,
            "title": format!("Claude Code: {}", project),
            "message": format!("Working...\n{}", cwd),
            "source": "claude-code",
            "status": "running",
        });
        post_notify(&payload);
        return;
    }

    let _ = fs::write(&lock_file, std::process::id().to_string());

    let tty = get_tty();
    let mut payload = serde_json::json!({
        "task_id": task_id,
        "title": format!("Claude Code: {}", project),
        "message": format!("Working...\n{}", cwd),
        "source": "claude-code",
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

fn handle_stop(task_id: &str, project: &str, cwd: &str) {
    let _ = fs::remove_file(flag_path(task_id, "registered"));
    let _ = fs::remove_file(flag_path(task_id, "pending"));

    let tty = get_tty();
    let mut payload = serde_json::json!({
        "task_id": task_id,
        "title": format!("Claude Code: {}", project),
        "message": "Session completed",
        "source": "claude-code",
        "status": "success",
        "workspace_path": cwd,
    });
    if let Some(ref t) = tty {
        payload["terminal_tty"] = Value::String(t.clone());
    }
    post_notify(&payload);
}

// ---------- helpers ----------

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
