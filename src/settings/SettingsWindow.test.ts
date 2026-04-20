import { describe, expect, test } from "vitest";
import { codeToKey, eventToShortcut, formatShortcut } from "./SettingsWindow";

function keyEvent(init: KeyboardEventInit): KeyboardEvent {
  return {
    code: init.code ?? "",
    key: init.key ?? "",
    metaKey: init.metaKey ?? false,
    ctrlKey: init.ctrlKey ?? false,
    altKey: init.altKey ?? false,
    shiftKey: init.shiftKey ?? false,
  } as KeyboardEvent;
}

describe("SettingsWindow helpers", () => {
  test("codeToKey 支持字母键", () => {
    expect(codeToKey(keyEvent({ code: "KeyA", key: "a" }))).toBe("A");
  });

  test("codeToKey 支持数字键", () => {
    expect(codeToKey(keyEvent({ code: "Digit5", key: "5" }))).toBe("5");
  });

  test("codeToKey 支持功能键", () => {
    expect(codeToKey(keyEvent({ code: "F12", key: "F12" }))).toBe("F12");
  });

  test("codeToKey 支持方向键", () => {
    expect(codeToKey(keyEvent({ code: "ArrowUp", key: "ArrowUp" }))).toBe("Up");
  });

  test("eventToShortcut 生成 CmdOrCtrl 快捷键", () => {
    expect(eventToShortcut(keyEvent({ metaKey: true, code: "KeyK", key: "k" }))).toBe("CmdOrCtrl+K");
  });

  test("eventToShortcut 支持多修饰键组合", () => {
    expect(
      eventToShortcut(keyEvent({ metaKey: true, shiftKey: true, code: "KeyP", key: "p" })),
    ).toBe("CmdOrCtrl+Shift+P");
  });

  test("eventToShortcut 纯修饰键返回 null", () => {
    expect(eventToShortcut(keyEvent({ key: "Meta", code: "MetaLeft", metaKey: true }))).toBe(null);
  });

  test("formatShortcut 转成人类可读格式", () => {
    expect(formatShortcut("CmdOrCtrl+Shift+K")).toBe("\u2318 \u21E7 K");
  });
});
