import { describe, expect, test } from "vitest";
import type { Session } from "../types";
import { isActive, projectName, sourceLabel, workspacePath } from "./SessionPanel";

function session(overrides: Partial<Session> = {}): Session {
  return {
    id: "session-1",
    task_id: "cc-123",
    external_session_id: "cc-session-123",
    title: "Claude Code: my-proj",
    message: "Working...",
    source: "claude-code",
    priority: "normal",
    status: "running",
    created_at: "2026-04-17T00:00:00Z",
    updated_at: "2026-04-17T00:00:00Z",
    terminal_tty: "/dev/ttys001",
    workspace_path: "/Users/tester/my-proj",
    failure_reason: null,
    ...overrides,
  };
}

describe("SessionPanel helpers", () => {
  test("projectName 从 title 中提取项目名", () => {
    expect(projectName(session())).toBe("my-proj");
  });

  test("projectName 在无前缀时回退原 title", () => {
    expect(projectName(session({ title: "plain" }))).toBe("plain");
  });

  test("sourceLabel 映射已知来源", () => {
    expect(sourceLabel("claude-code")).toBe("Claude Code");
    expect(sourceLabel("cursor")).toBe("Cursor");
    expect(sourceLabel("codex")).toBe("Codex");
  });

  test("sourceLabel 对空值返回空字符串", () => {
    expect(sourceLabel(null)).toBe("");
  });

  test("workspacePath 将用户目录缩写为 ~", () => {
    expect(workspacePath(session())).toBe("~/my-proj");
  });

  test("isActive 对 running 和 pending 返回 true", () => {
    expect(isActive(session({ status: "running" }))).toBe(true);
    expect(isActive(session({ status: "pending" }))).toBe(true);
  });

  test("isActive 对 idle 和 last_failed 返回 false", () => {
    expect(isActive(session({ status: "idle" }))).toBe(false);
    expect(isActive(session({ status: "last_failed" }))).toBe(false);
  });
});
