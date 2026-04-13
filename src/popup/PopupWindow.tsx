import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import SourceIcon from "../icons/SourceIcon";
import type { Task } from "../types";
import "./popup.css";

function timeAgo(dateStr: string): string {
  const now = Date.now();
  const then = new Date(dateStr).getTime();
  const seconds = Math.floor((now - then) / 1000);
  if (seconds < 60) return "刚刚";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  return `${hours} 小时前`;
}

export default function PopupWindow() {
  const [task, setTask] = useState<Task | null>(null);
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const label = getCurrentWebviewWindow().label;
    const id = label.replace("popup-", "");

    invoke<Task | null>("get_notification_by_id", { id }).then((data) => {
      if (data) {
        setTask(data);
        requestAnimationFrame(() => {
          setVisible(true);
        });
      }
    });
  }, []);

  const handleClick = async () => {
    if (!task) return;
    // Focus terminal if possible, then auto-close popup
    if (task.terminal_tty || task.workspace_path) {
      await invoke("focus_task_terminal", { id: task.id });
    }
    setVisible(false);
    await invoke("close_popup_window", { id: task.id });
  };

  if (!task) {
    return <div className="popup-container" />;
  }

  const statusClass = task.status === "failed" ? "failed" : task.status === "success" ? "success" : "";
  const canFocus = !!(task.terminal_tty || task.workspace_path);

  return (
    <div
      className={`popup-container ${visible ? "show" : ""} ${statusClass}`}
      onClick={handleClick}
      style={{ cursor: "pointer" }}
    >
      <div className="popup-glow" />
      <div className="popup-body">
        <div className="popup-left">
          <SourceIcon source={task.source} status={task.status} />
        </div>
        <div className="popup-right">
          <div className="popup-header">
            <span className="popup-title">{task.title}</span>
            {task.source && (
              <span className="popup-source">{task.source}</span>
            )}
          </div>
          <div className="popup-message">{task.message}</div>
          <div className="popup-footer">
            <span className="popup-time">{timeAgo(task.created_at)}</span>
            {canFocus && (
              <span className="popup-hint">点击跳转</span>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
