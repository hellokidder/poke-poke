import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import SourceIcon from "../icons/SourceIcon";
import { useT } from "../i18n/context";
import type { Session } from "../types";
import "./popup.css";

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
    return t("time.hours_ago", { n: hours });
  };
}

export default function PopupWindow() {
  const t = useT();
  const timeAgo = useTimeAgo();
  const [session, setSession] = useState<Session | null>(null);
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const label = getCurrentWebviewWindow().label;
    const id = label.replace("popup-", "");

    invoke<Session | null>("get_session_by_id", { id }).then((data) => {
      if (data) {
        setSession(data);
        requestAnimationFrame(() => {
          setVisible(true);
        });
      }
    });
  }, []);

  const handleClick = async () => {
    if (!session) return;
    if (session.terminal_tty || session.workspace_path) {
      await invoke("focus_session_terminal", { id: session.id });
    }
    setVisible(false);
    await invoke("close_popup_window", { id: session.id });
  };

  if (!session) {
    return <div className="popup-container" />;
  }

  const statusClass =
    session.status === "success" ? "success"
    : session.status === "failure" ? "failure"
    : session.status === "pending" ? "pending"
    : "";
  const canFocus = !!(session.terminal_tty || session.workspace_path);

  return (
    <div
      className={`popup-container ${visible ? "show" : ""} ${statusClass}`}
      onClick={handleClick}
      style={{ cursor: "pointer" }}
    >
      <div className="popup-glow" />
      <div className="popup-body">
        <div className="popup-left">
          <SourceIcon source={session.source} status={session.status} colorSeed={session.task_id} />
        </div>
        <div className="popup-right">
          <div className="popup-header">
            <span className="popup-title">{session.title}</span>
            {session.source && (
              <span className="popup-source">{session.source}</span>
            )}
          </div>
          <div className="popup-message">{session.message}</div>
          <div className="popup-footer">
            <span className="popup-time">{timeAgo(session.created_at)}</span>
            {canFocus && (
              <span className="popup-hint">{t("popup.click_to_jump")}</span>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
