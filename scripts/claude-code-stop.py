#!/usr/bin/env python3
"""Claude Code Stop hook：对话完成后向本机 PokePoke POST /notify。
支持 Stop / SubagentStop 事件；使用与 PreToolUse hook 一致的 task_id。
"""
import json
import os
import subprocess
import sys
import urllib.error
import urllib.request

POKE_PORTS = (9876, 9877)
HANDLED_EVENTS = frozenset({"Stop", "SubagentStop"})
LOCK_DIR = "/tmp"


def log(msg: str) -> None:
    sys.stderr.write(f"[pokepoke-cc] {msg}\n")
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
    last_err: str | None = None
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
                    log(f"已通知 PokePoke: {url} HTTP {resp.status}")
                    return True
        except urllib.error.HTTPError as e:
            last_err = f"{url} HTTP {e.code}: {e.reason}"
        except (urllib.error.URLError, TimeoutError, OSError) as e:
            last_err = f"{url} -> {e!r}"

    log(
        "未能连上 PokePoke（9876/9877），请确认应用已启动。"
        + (f" 最后错误: {last_err}" if last_err else "")
    )
    return False


def main() -> None:
    raw = sys.stdin.read()
    if not raw.strip():
        log("stdin 为空，跳过")
        print("{}", flush=True)
        return

    try:
        data = json.loads(raw)
    except json.JSONDecodeError as e:
        log(f"JSON 解析失败: {e}")
        print("{}", flush=True)
        return

    hook_event = data.get("hook_event_name", "")
    if hook_event not in HANDLED_EVENTS:
        log(f"事件 {hook_event!r} 不在处理列表，跳过")
        print("{}", flush=True)
        return

    session_id = data.get("session_id", "unknown")
    task_id = f"cc-{session_id}"

    # Clean up the lock file so a new session can be registered
    lock_file = os.path.join(LOCK_DIR, f"pokepoke-cc-{session_id}.registered")
    try:
        os.remove(lock_file)
    except OSError:
        pass  # Already gone or never created

    cwd = os.getcwd()
    project = os.path.basename(cwd)
    tty = get_tty()

    if hook_event == "SubagentStop":
        title = f"Claude Code 子任务完成：{project}"
    else:
        title = f"Claude Code 完成：{project}"

    message = f"路径: {cwd}\nSession: {session_id[:8]}..."

    payload = {
        "title": title,
        "message": message,
        "task_id": task_id,
        "source": "claude-code",
        "status": "success",
        "workspace_path": cwd,
    }
    if tty:
        payload["terminal_tty"] = tty

    log(f"发送完成通知 session={session_id[:8]}... tty={tty}")
    post_notify(payload)

    print("{}", flush=True)


if __name__ == "__main__":
    main()
