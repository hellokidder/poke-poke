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

function StatusIndicator({ status }: { status: TaskStatus }) {
  switch (status) {
    case "pending":
      return <div className="status-indicator pending"><div className="pulse-dot" /></div>;
    case "running":
      return <div className="status-indicator running"><div className="spinner" /></div>;
    case "success":
      return <div className="status-indicator success">✓</div>;
    case "failed":
      return <div className="status-indicator failed">✗</div>;
  }
}

const sectionConfig = [
  { key: "active", label: "进行中", filter: (t: Task) => t.status === "pending" || t.status === "running" },
  { key: "failed", label: "失败", filter: (t: Task) => t.status === "failed" && !t.read },
  { key: "success", label: "成功", filter: (t: Task) => t.status === "success" && !t.read },
  { key: "read", label: "已读", filter: (t: Task) => t.read && t.status !== "pending" && t.status !== "running" },
] as const;

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

  const handleMarkRead = async (id: string) => {
    await invoke("mark_notification_read", { id });
    loadTasks();
  };

  const handleMarkAllRead = async () => {
    await invoke("mark_all_read");
    loadTasks();
  };

  const unreadCount = tasks.filter((t) => !t.read && (t.status === "success" || t.status === "failed")).length;

  return (
    <div className="panel-container">
      <div className="panel-header">
        <h1 className="panel-title">Poke Poke</h1>
        {unreadCount > 0 && (
          <button className="panel-mark-all" onClick={handleMarkAllRead}>
            全部已读
          </button>
        )}
      </div>

      <div className="panel-list">
        {tasks.length === 0 ? (
          <div className="panel-empty">暂无任务</div>
        ) : (
          sectionConfig.map(({ key, label, filter }) => {
            const items = tasks.filter(filter);
            if (items.length === 0) return null;
            return (
              <div key={key} className={`panel-section section-${key}`}>
                <div className="section-header">
                  <span>{label}</span>
                  <span className="section-count">{items.length}</span>
                </div>
                {items.map((t) => (
                  <div
                    key={t.id}
                    className={`panel-item ${t.read ? "read" : ""}`}
                    onClick={() => !t.read && t.status !== "pending" && t.status !== "running" && handleMarkRead(t.id)}
                  >
                    <StatusIndicator status={t.status} />
                    <SourceIcon source={t.source} status={t.status} />
                    <div className="item-content">
                      <div className="item-header">
                        <span className="item-title">{t.title}</span>
                        {t.source && <span className="item-source">{t.source}</span>}
                      </div>
                      <div className="item-message">{t.message}</div>
                      <div className="item-time">{timeAgo(t.updated_at)}</div>
                    </div>
                  </div>
                ))}
              </div>
            );
          })
        )}
      </div>

      <div className="panel-footer">
        {unreadCount > 0 ? `${unreadCount} 条未读` : "全部已读"}
      </div>
    </div>
  );
}
