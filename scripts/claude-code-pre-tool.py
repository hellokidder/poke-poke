#!/usr/bin/env python3
"""Claude Code PreToolUse hook: registers the session as "running" in PokePoke.
Fires on every tool call, but uses a lock file so only the first call registers.
"""
import json
import os
import subprocess
import sys
import urllib.error
import urllib.request

POKE_PORTS = (9876, 9877)
LOCK_DIR = "/tmp"


def log(msg: str) -> None:
    sys.stderr.write(f"[pokepoke-pre] {msg}\n")
    sys.stderr.flush()


def get_tty() -> str | None:
    """Walk up process tree to find the controlling terminal tty path."""
    pid = os.getpid()
    for _ in range(8):
        try:
            out = subprocess.check_output(
                ["ps", "-p", str(pid), "-o", "tty=,ppid="],
                text=True, timeout=2
            ).strip()
            parts = out.split()
            if not parts:
                break
            tty_name = parts[0]
            ppid = int(parts[1]) if len(parts) > 1 else 0
            if tty_name and tty_name not in ("??", ""):
                return f"/dev/{tty_name}"
            if ppid > 1:
                pid = ppid
            else:
                break
        except Exception:
            break
    return None


def post_notify(payload: dict) -> bool:
    body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    for port in POKE_PORTS:
        url = f"http://127.0.0.1:{port}/notify"
        req = urllib.request.Request(
            url, data=body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=2) as resp:
                if 200 <= resp.status < 300:
                    log(f"通知成功: {url} HTTP {resp.status}")
                    return True
        except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, OSError) as e:
            log(f"{url} 失败: {e!r}")
    return False


def main() -> None:
    raw = sys.stdin.read()
    if not raw.strip():
        print("{}", flush=True)
        return

    try:
        data = json.loads(raw)
    except json.JSONDecodeError as e:
        log(f"JSON 解析失败: {e}")
        print("{}", flush=True)
        return

    hook_event = data.get("hook_event_name", "")
    if hook_event != "PreToolUse":
        print("{}", flush=True)
        return

    session_id = data.get("session_id", "unknown")
    lock_file = os.path.join(LOCK_DIR, f"pokepoke-cc-{session_id}.registered")

    # Only register once per session
    if os.path.exists(lock_file):
        print("{}", flush=True)
        return

    # Create lock file immediately to prevent duplicate registrations
    try:
        with open(lock_file, "w") as f:
            f.write(str(os.getpid()))
    except OSError as e:
        log(f"无法创建锁文件: {e}")
        print("{}", flush=True)
        return

    cwd = os.getcwd()
    project = os.path.basename(cwd)
    tty = get_tty()
    tool_name = data.get("tool_name", "")

    log(f"注册会话 {session_id[:8]}... tty={tty} tool={tool_name}")

    payload = {
        "title": f"Claude Code：{project}",
        "message": f"正在处理任务...\n路径: {cwd}",
        "task_id": f"cc-{session_id}",
        "source": "claude-code",
        "status": "running",
        "workspace_path": cwd,
    }
    if tty:
        payload["terminal_tty"] = tty

    if not post_notify(payload):
        log("未能连上 PokePoke，请确认应用已启动")

    print("{}", flush=True)


if __name__ == "__main__":
    main()
