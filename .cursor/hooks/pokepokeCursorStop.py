#!/usr/bin/env python3
"""Cursor 钩子：向本机 PokePoke POST /notify。支持 stop / sessionEnd；stderr 打日志便于 Hooks 通道排查。"""
import json
import sys
import uuid
import urllib.error
import urllib.request

POKE_PORTS = (9876, 9877)
HANDLED_EVENTS = frozenset({"stop", "sessionEnd"})


def pick(d: dict, *keys: str) -> str:
    for k in keys:
        v = d.get(k)
        if v is not None and v != "":
            return str(v)
    return ""


def log(msg: str) -> None:
    sys.stderr.write(f"[pokepoke-hook] {msg}\n")
    sys.stderr.flush()


def main() -> None:
    raw = sys.stdin.read()
    if not raw.strip():
        log("stdin 为空，跳过（请确认 Cursor 已把 JSON 传给钩子）")
        print("{}", flush=True)
        return

    try:
        data = json.loads(raw)
    except json.JSONDecodeError as e:
        log(f"JSON 解析失败: {e}")
        print("{}", flush=True)
        return

    hook_event = pick(data, "hook_event_name", "hookEventName") or ""
    if hook_event not in HANDLED_EVENTS:
        log(f"事件 {hook_event!r} 不在处理列表 {HANDLED_EVENTS}，跳过")
        print("{}", flush=True)
        return

    hook_status = pick(data, "status", "hookStatus") or "completed"
    generation_id = pick(data, "generation_id", "generationId")
    conversation_id = pick(data, "conversation_id", "conversationId")
    model = pick(data, "model", "modelName")
    roots = data.get("workspace_roots") or data.get("workspaceRoots") or []
    workspace = roots[0] if isinstance(roots, list) and roots else ""

    if hook_event == "sessionEnd":
        poke_status = "success"
        title = "Cursor：会话已结束"
    elif hook_status == "completed":
        poke_status = "success"
        title = "Cursor：本轮 Agent 已结束"
    elif hook_status == "aborted":
        poke_status = "failed"
        title = "Cursor：本轮已中止"
    else:
        poke_status = "failed"
        title = "Cursor：本轮出错结束"

    lines = [f"事件: {hook_event}", f"hook 状态: {hook_status}", f"模型: {model}"]
    if workspace:
        lines.append(f"工作区: {workspace}")
    message = "\n".join(lines)

    suffix = uuid.uuid4().hex[:10]
    base = generation_id or conversation_id or "na"
    task_id = f"cursor-{hook_event}-{base}-{suffix}"

    body = json.dumps(
        {
            "title": title,
            "message": message,
            "task_id": task_id,
            "source": "cursor",
            "status": poke_status,
        },
        ensure_ascii=False,
    ).encode("utf-8")

    ok = False
    last_err: str | None = None
    for port in POKE_PORTS:
        url = f"http://127.0.0.1:{port}/notify"
        req = urllib.request.Request(
            url,
            data=body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=2) as resp:
                if 200 <= resp.status < 300:
                    log(f"已通知 PokePoke: {url} HTTP {resp.status}")
                    ok = True
                    break
        except urllib.error.HTTPError as e:
            last_err = f"{url} HTTP {e.code}: {e.reason}"
        except (urllib.error.URLError, TimeoutError, OSError) as e:
            last_err = f"{url} -> {e!r}"

    if not ok:
        log(
            "未能连上 PokePoke /notify（9876/9877）。"
            "请确认已用「npm run tauri dev」或安装版启动应用，且本机 python3 可访问 127.0.0.1。"
            + (f" 最后错误: {last_err}" if last_err else "")
        )

    print("{}", flush=True)


if __name__ == "__main__":
    main()
