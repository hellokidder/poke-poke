import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import SourceIcon from "../icons/SourceIcon";
import type { Task, TaskStatus } from "../types";
import "./panel.css";

function timeAgo(dateStr: string): string {
  const now = Date.now();
  const then = new Date(dateStr).getTime();
  const seconds = Math.floor((now - then) / 1000);
  if (seconds < 60) return "刚刚";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  return `${days} 天前`;
}

function StatusDot({ status }: { status: TaskStatus }) {
  const config: Record<TaskStatus, { color: string; label: string; animate: boolean }> = {
    running: { color: "#4ade80", label: "运行中", animate: true },
    pending: { color: "#facc15", label: "等待中", animate: true },
    success: { color: "rgba(255,255,255,0.25)", label: "已完成", animate: false },
    failed: { color: "#f87171", label: "失败", animate: false },
  };
  const c = config[status];
  return (
    <span className={`status-dot ${c.animate ? "active" : ""}`} title={c.label}>
      <span className="dot-inner" style={{ background: c.color }} />
    </span>
  );
}

/** Extract short project name from title like "Claude Code: my-project" */
function projectName(task: Task): string {
  const match = task.title.match(/:\s*(.+)/);
  return match ? match[1] : task.title;
}

/** Extract short workspace display path */
function workspacePath(task: Task): string {
  if (!task.workspace_path) return "";
  // Show ~ for home dir, keep last 2 segments
  const p = task.workspace_path.replace(/^\/Users\/[^/]+/, "~");
  return p;
}

function isActive(t: Task): boolean {
  return t.status === "running" || t.status === "pending";
}

export default function NotificationPanel() {
  const [tasks, setTasks] = useState<Task[]>([]);

  const loadTasks = async () => {
    const data = await invoke<Task[]>("get_notifications");
    setTasks(data);
  };

  useEffect(() => {
    loadTasks();
    const unlisten = listen("notifications-updated", () => {
      loadTasks();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleClick = (id: string) => {
    invoke("open_task_source", { id });
  };

  // Sort by created_at desc (newest first, fixed order)
  const sorted = [...tasks].sort((a, b) =>
    new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
  );

  const activeCount = sorted.filter(isActive).length;

  return (
    <div className="panel-container">
      <div className="panel-header">
        <h1 className="panel-title">Poke Poke</h1>
        <span className="connection-count">
          {activeCount > 0 ? `${activeCount} 个活跃` : "无活跃连接"}
        </span>
      </div>

      <div className="panel-list">
        {sorted.length === 0 ? (
          <div className="panel-empty">暂无连接的会话</div>
        ) : (
          sorted.map((t) => (
            <div
              key={t.id}
              className={`session-item ${isActive(t) ? "" : "inactive"}`}
              onClick={() => handleClick(t.id)}
            >
              <SourceIcon source={t.source} status={t.status} colorSeed={t.task_id} />
              <div className="session-info">
                <div className="session-header">
                  <span className="session-project">{projectName(t)}</span>
                  <StatusDot status={t.status} />
                </div>
                <div className="session-path">{workspacePath(t)}</div>
                <div className="session-time">{timeAgo(t.updated_at)}</div>
              </div>
            </div>
          ))
        )}
      </div>

      <div className="panel-footer">
        {sorted.length} 个会话
      </div>
    </div>
  );
}
