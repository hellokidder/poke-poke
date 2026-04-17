export type SessionStatus = "pending" | "running" | "success" | "failure";

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
}
