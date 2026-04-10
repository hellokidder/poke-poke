export type TaskStatus = "pending" | "running" | "success" | "failed";

export interface Task {
  id: string;
  task_id: string;
  title: string;
  message: string;
  source: string | null;
  priority: "normal" | "high";
  status: TaskStatus;
  read: boolean;
  created_at: string;
  updated_at: string;
}
