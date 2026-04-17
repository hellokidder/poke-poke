export type SessionStatus = "pending" | "running" | "idle" | "last_failed";

export interface Session {
  id: string;
  task_id: string;
  title: string;
  message: string;
  source: string | null;
  priority: "normal" | "high";
  status: SessionStatus;
  created_at: string;
  updated_at: string;
  terminal_tty: string | null;
  workspace_path: string | null;
  failure_reason: string | null;
}
