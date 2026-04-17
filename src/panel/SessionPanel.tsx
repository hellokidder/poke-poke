import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import SourceIcon from "../icons/SourceIcon";
import { useT } from "../i18n/context";
import type { Session, SessionStatus } from "../types";
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

function StatusDot({ status }: { status: SessionStatus }) {
  const t = useT();
  const config: Record<SessionStatus, { color: string; label: string; animate: boolean }> = {
    running: { color: "#4ade80", label: t("status.running"), animate: true },
    pending: { color: "#facc15", label: t("status.pending"), animate: true },
    success: { color: "rgba(255,255,255,0.25)", label: t("status.success"), animate: false },
    failure: { color: "#f87171", label: t("status.failure"), animate: false },
  };
  const c = config[status];
  return (
    <span className={`status-dot ${c.animate ? "active" : ""}`} title={c.label}>
      <span className="dot-inner" style={{ background: c.color }} />
    </span>
  );
}

function projectName(session: Session): string {
  const match = session.title.match(/:\s*(.+)/);
  return match ? match[1] : session.title;
}

function sourceLabel(source: string | null): string {
  switch (source) {
    case "claude-code": return "Claude Code";
    case "cursor": return "Cursor";
    case "codex": return "Codex";
    default: return source || "";
  }
}

function workspacePath(session: Session): string {
  if (!session.workspace_path) return "";
  return session.workspace_path.replace(/^\/Users\/[^/]+/, "~");
}

function isActive(s: Session): boolean {
  return s.status === "running" || s.status === "pending";
}

export default function SessionPanel() {
  const t = useT();
  const timeAgo = useTimeAgo();
  const [sessions, setSessions] = useState<Session[]>([]);

  const loadSessions = async () => {
    const data = await invoke<Session[]>("get_sessions");
    setSessions(data);
  };

  useEffect(() => {
    loadSessions();
    const unlisten = listen("sessions-updated", () => {
      loadSessions();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleClick = (id: string) => {
    invoke("open_session_source", { id });
  };

  const openSettings = () => {
    invoke("open_settings_window");
  };

  const sorted = [...sessions].sort((a, b) =>
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
          sorted.map((session) => (
            <div
              key={session.id}
              className={`session-item ${isActive(session) ? "" : "inactive"}`}
              onClick={() => handleClick(session.id)}
            >
              <SourceIcon source={session.source} status={session.status} colorSeed={session.task_id} />
              <div className="session-info">
                <div className="session-header">
                  <span className="session-project">{projectName(session)}</span>
                  <div className="session-header-right">
                    <span className="session-source">{sourceLabel(session.source)}</span>
                    <StatusDot status={session.status} />
                  </div>
                </div>
                <div className="session-path">{workspacePath(session)}</div>
                <div className="session-time">{timeAgo(session.updated_at)}</div>
              </div>
              {!isActive(session) && (
                <button
                  className="session-delete-btn"
                  title={t("panel.delete")}
                  onClick={(e) => {
                    e.stopPropagation();
                    invoke("remove_session", { id: session.id });
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
