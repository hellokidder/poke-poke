import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import SourceIcon from "../icons/SourceIcon";
import { useT } from "../i18n/context";
import type { Task, TaskStatus } from "../types";
import "./panel.css";

function useTimeAgo() {
  const t = useT();
  return (dateStr: string): string => {
    const now = Date.now();
    const then = new Date(dateStr).getTime();
    const seconds = Math.floor((now - then) / 1000);
    if (seconds < 60) return t("time.just_now");
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return t("time.minutes_ago", { n: minutes });
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return t("time.hours_ago", { n: hours });
    const days = Math.floor(hours / 24);
    return t("time.days_ago", { n: days });
  };
}

function StatusDot({ status }: { status: TaskStatus }) {
  const t = useT();
  const config: Record<TaskStatus, { color: string; label: string; animate: boolean }> = {
    running: { color: "#4ade80", label: t("status.running"), animate: true },
    pending: { color: "#facc15", label: t("status.pending"), animate: true },
    success: { color: "rgba(255,255,255,0.25)", label: t("status.success"), animate: false },
    failed: { color: "#f87171", label: t("status.failed"), animate: false },
  };
  const c = config[status];
  return (
    <span className={`status-dot ${c.animate ? "active" : ""}`} title={c.label}>
      <span className="dot-inner" style={{ background: c.color }} />
    </span>
  );
}

function projectName(task: Task): string {
  const match = task.title.match(/:\s*(.+)/);
  return match ? match[1] : task.title;
}

function sourceLabel(source: string | null): string {
  switch (source) {
    case "claude-code": return "Claude Code";
    case "cursor": return "Cursor";
    case "codex": return "Codex";
    default: return source || "";
  }
}

function workspacePath(task: Task): string {
  if (!task.workspace_path) return "";
  return task.workspace_path.replace(/^\/Users\/[^/]+/, "~");
}

function isActive(t: Task): boolean {
  return t.status === "running" || t.status === "pending";
}

export default function NotificationPanel() {
  const t = useT();
  const timeAgo = useTimeAgo();
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

  const openSettings = () => {
    invoke("open_settings_window");
  };

  const sorted = [...tasks].sort((a, b) =>
    new Date(a.created_at).getTime() - new Date(b.created_at).getTime()
  );

  const activeCount = sorted.filter(isActive).length;

  return (
    <div className="panel-container">
      <div className="panel-header">
        <h1 className="panel-title">Poke Poke</h1>
        <span className="connection-count">
          {activeCount > 0 ? t("panel.active_count", { n: activeCount }) : t("panel.no_active")}
        </span>
      </div>

      <div className="panel-list">
        {sorted.length === 0 ? (
          <div className="panel-empty">{t("panel.empty")}</div>
        ) : (
          sorted.map((task) => (
            <div
              key={task.id}
              className={`session-item ${isActive(task) ? "" : "inactive"}`}
              onClick={() => handleClick(task.id)}
            >
              <SourceIcon source={task.source} status={task.status} colorSeed={task.task_id} />
              <div className="session-info">
                <div className="session-header">
                  <span className="session-project">{projectName(task)}</span>
                  <div className="session-header-right">
                    <span className="session-source">{sourceLabel(task.source)}</span>
                    <StatusDot status={task.status} />
                  </div>
                </div>
                <div className="session-path">{workspacePath(task)}</div>
                <div className="session-time">{timeAgo(task.updated_at)}</div>
              </div>
              {!isActive(task) && (
                <button
                  className="session-delete-btn"
                  title={t("panel.delete")}
                  onClick={(e) => {
                    e.stopPropagation();
                    invoke("remove_notification", { id: task.id });
                  }}
                >
                  <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                    <path d="M3 3L9 9M9 3L3 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
                  </svg>
                </button>
              )}
            </div>
          ))
        )}
      </div>

      <div className="panel-footer">
        <span>{t("panel.sessions", { n: sorted.length })}</span>
        <button className="settings-btn" onClick={openSettings} title="Settings">
          <svg width="15" height="15" viewBox="0 0 16 16" fill="none">
            <path d="M6.5 1.5h3l.4 1.7.7.3 1.5-.8 2.1 2.1-.8 1.5.3.7 1.7.4v3l-1.7.4-.3.7.8 1.5-2.1 2.1-1.5-.8-.7.3-.4 1.7h-3l-.4-1.7-.7-.3-1.5.8-2.1-2.1.8-1.5-.3-.7-1.7-.4v-3l1.7-.4.3-.7-.8-1.5 2.1-2.1 1.5.8.7-.3z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round"/>
            <circle cx="8" cy="8" r="2" stroke="currentColor" strokeWidth="1.2"/>
          </svg>
        </button>
      </div>
    </div>
  );
}
