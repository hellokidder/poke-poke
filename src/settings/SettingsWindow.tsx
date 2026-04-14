import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { useT } from "../i18n/context";
import "./settings.css";

interface Settings {
  panel_shortcut: string | null;
  alert_sound: string;
  popup_timeout: number;
  locale: string;
  auto_start: boolean;
  session_retention_hours: number;
}

const MOD_KEYS = new Set(["Meta", "Control", "Alt", "Shift"]);

/** Map e.code to a key name the global-hotkey library understands */
function codeToKey(e: KeyboardEvent): string | null {
  const { code, key } = e;
  // Letters: KeyA → A
  if (code.startsWith("Key")) return code.slice(3);
  // Digits: Digit0 → 0
  if (code.startsWith("Digit")) return code.slice(5);
  // Numpad: Numpad0 → Num0
  if (code.startsWith("Numpad")) return `Num${code.slice(6)}`;
  // Function keys: F1-F24
  if (/^F\d+$/.test(code)) return code;
  // Special keys
  const map: Record<string, string> = {
    Space: "Space",
    Minus: "-",
    Equal: "=",
    BracketLeft: "[",
    BracketRight: "]",
    Backslash: "\\",
    Semicolon: ";",
    Quote: "'",
    Comma: ",",
    Period: ".",
    Slash: "/",
    Backquote: "`",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Escape: "Escape",
    Enter: "Return",
    Backspace: "Backspace",
    Delete: "Delete",
    Tab: "Tab",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
  };
  if (map[code]) return map[code];
  // Fallback: use key if it's a single printable char
  if (key.length === 1) return key.toUpperCase();
  return null;
}

function eventToShortcut(e: KeyboardEvent): string | null {
  if (MOD_KEYS.has(e.key)) return null;
  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) parts.push("CmdOrCtrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (parts.length === 0) return null;
  const key = codeToKey(e);
  if (!key) return null;
  parts.push(key);
  return parts.join("+");
}

function formatShortcut(s: string): string {
  return s
    .replace(/CmdOrCtrl/g, "\u2318")
    .replace(/Alt/g, "\u2325")
    .replace(/Shift/g, "\u21E7")
    .replace(/\+/g, " ");
}

const TIMEOUT_OPTIONS = [0, 10, 30];
const RETENTION_OPTIONS = [
  { value: 1, labelKey: "settings.hours", n: 1 },
  { value: 24, labelKey: "settings.hours", n: 24 },
  { value: 168, labelKey: "settings.days", n: 7 },
  { value: 0, labelKey: "settings.forever", n: 0 },
];

export default function SettingsWindow() {
  const t = useT();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [sounds, setSounds] = useState<string[]>([]);
  const [recording, setRecording] = useState(false);
  const [saved, setSaved] = useState(false);
  const [autoStartEnabled, setAutoStartEnabled] = useState(false);
  const [soundOpen, setSoundOpen] = useState(false);
  const soundRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invoke<Settings>("get_settings").then(setSettings);
    invoke<string[]>("list_system_sounds").then(setSounds);
    isEnabled().then(setAutoStartEnabled);
  }, []);

  // ESC to close window (when not recording shortcut) or close dropdown
  useEffect(() => {
    if (recording) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (soundOpen) {
          setSoundOpen(false);
        } else {
          invoke("close_settings_window");
        }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [recording, soundOpen]);

  // Click outside to close dropdown
  useEffect(() => {
    if (!soundOpen) return;
    const handler = (e: MouseEvent) => {
      if (soundRef.current && !soundRef.current.contains(e.target as Node)) {
        setSoundOpen(false);
      }
    };
    window.addEventListener("mousedown", handler);
    return () => window.removeEventListener("mousedown", handler);
  }, [soundOpen]);

  useEffect(() => {
    if (!recording) return;
    const handler = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setRecording(false);
        return;
      }
      const shortcut = eventToShortcut(e);
      if (shortcut && settings) {
        setRecording(false);
        saveField("panel_shortcut", shortcut);
      }
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [recording, settings]);

  if (!settings) return null;

  const saveField = async (key: keyof Settings, value: unknown) => {
    const next = { ...settings, [key]: value } as Settings;
    setSettings(next);
    await invoke("save_settings", { settings: next });
    setSaved(true);
    setTimeout(() => setSaved(false), 1200);
  };

  const handleSoundChange = (val: string) => {
    saveField("alert_sound", val);
  };

  const handlePreview = () => {
    if (settings.alert_sound === "mute") return;
    const name = settings.alert_sound.replace("system:", "");
    invoke("preview_sound", { name });
  };

  const handleTimeoutChange = (val: number) => {
    saveField("popup_timeout", val);
  };

  const handleLocaleChange = (val: string) => {
    saveField("locale", val);
  };

  const handleAutoStartToggle = async () => {
    const next = !autoStartEnabled;
    if (next) {
      await enable();
    } else {
      await disable();
    }
    setAutoStartEnabled(next);
    saveField("auto_start", next);
  };

  const handleClearShortcut = () => {
    saveField("panel_shortcut", null);
  };

  return (
    <div className="settings-window">
      <div className="settings-window-header">
        <h1 className="settings-window-title">{t("settings.title")}</h1>
        <button className="sw-close-btn" onClick={() => invoke("close_settings_window")}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <path d="M4.5 4.5L11.5 11.5M11.5 4.5L4.5 11.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
          </svg>
        </button>
      </div>

      <div className="settings-window-body">
        {/* Sound */}
        <div className="sw-section">
          <div className="sw-label">{t("settings.sound")}</div>
          <div className="sw-desc">{t("settings.sound_desc")}</div>
          <div className="sw-sound-row">
            <div className="sw-dropdown" ref={soundRef}>
              <div
                className={`sw-dropdown-trigger ${soundOpen ? "open" : ""}`}
                onClick={() => setSoundOpen(!soundOpen)}
              >
                <span>{settings.alert_sound === "mute" ? t("settings.mute") : settings.alert_sound.replace("system:", "")}</span>
                <svg className="sw-dropdown-arrow" width="12" height="12" viewBox="0 0 12 12" fill="none">
                  <path d="M3 4.5L6 7.5L9 4.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                </svg>
              </div>
              {soundOpen && (
                <div className="sw-dropdown-menu">
                  <div
                    className={`sw-dropdown-item ${settings.alert_sound === "mute" ? "active" : ""}`}
                    onClick={() => { handleSoundChange("mute"); setSoundOpen(false); }}
                  >
                    {t("settings.mute")}
                  </div>
                  {sounds.map((s) => (
                    <div
                      key={s}
                      className={`sw-dropdown-item ${settings.alert_sound === `system:${s}` ? "active" : ""}`}
                      onClick={() => { handleSoundChange(`system:${s}`); setSoundOpen(false); }}
                    >
                      {s}
                    </div>
                  ))}
                </div>
              )}
            </div>
            {settings.alert_sound !== "mute" && (
              <button className="sw-preview-btn" onClick={handlePreview}>
                {t("settings.preview")}
              </button>
            )}
          </div>
        </div>

        {/* Popup timeout */}
        <div className="sw-section">
          <div className="sw-label">{t("settings.timeout")}</div>
          <div className="sw-desc">{t("settings.timeout_desc")}</div>
          <div className="sw-radio-group">
            {TIMEOUT_OPTIONS.map((val) => (
              <div
                key={val}
                className={`sw-radio-option ${settings.popup_timeout === val ? "active" : ""}`}
                onClick={() => handleTimeoutChange(val)}
              >
                {val === 0 ? t("settings.never") : t("settings.seconds", { n: val })}
              </div>
            ))}
          </div>
        </div>

        {/* Session retention */}
        <div className="sw-section">
          <div className="sw-label">{t("settings.retention")}</div>
          <div className="sw-desc">{t("settings.retention_desc")}</div>
          <div className="sw-radio-group">
            {RETENTION_OPTIONS.map((opt) => (
              <div
                key={opt.value}
                className={`sw-radio-option ${settings.session_retention_hours === opt.value ? "active" : ""}`}
                onClick={() => saveField("session_retention_hours", opt.value)}
              >
                {opt.value === 0 ? t("settings.forever") : t(opt.labelKey, { n: opt.n })}
              </div>
            ))}
          </div>
        </div>

        {/* Language */}
        <div className="sw-section">
          <div className="sw-label">{t("settings.language")}</div>
          <div className="sw-desc">{t("settings.language_desc")}</div>
          <div className="sw-radio-group">
            <div
              className={`sw-radio-option ${settings.locale === "zh" ? "active" : ""}`}
              onClick={() => handleLocaleChange("zh")}
            >
              中文
            </div>
            <div
              className={`sw-radio-option ${settings.locale === "en" ? "active" : ""}`}
              onClick={() => handleLocaleChange("en")}
            >
              English
            </div>
          </div>
        </div>

        {/* Auto-start */}
        <div className="sw-section">
          <div className="sw-section-header">
            <div>
              <div className="sw-label">{t("settings.autostart")}</div>
              <div className="sw-desc" style={{ marginBottom: 0 }}>
                {t("settings.autostart_desc")}
              </div>
            </div>
            <div
              className={`sw-toggle ${autoStartEnabled ? "on" : ""}`}
              onClick={handleAutoStartToggle}
            >
              <div className="sw-toggle-knob" />
            </div>
          </div>
        </div>

        {/* Panel shortcut */}
        <div className="sw-section">
          <div className="sw-label">{t("settings.shortcut")}</div>
          <div className="sw-desc">{t("settings.shortcut_desc")}</div>
          <div className="sw-shortcut-row">
            <div
              className={`sw-shortcut-input ${recording ? "recording" : ""}`}
              onClick={() => setRecording(true)}
              tabIndex={0}
            >
              {recording
                ? t("settings.press_keys")
                : settings.panel_shortcut
                  ? formatShortcut(settings.panel_shortcut)
                  : t("settings.click_to_set")}
            </div>
            {settings.panel_shortcut && !recording && (
              <button className="sw-shortcut-clear" onClick={handleClearShortcut} title="Clear">
                <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                  <path d="M4 4L10 10M10 4L4 10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
                </svg>
              </button>
            )}
          </div>
        </div>
      </div>

      {saved && (
        <div className="settings-window-footer">
          <span className="sw-saved-indicator">{t("settings.saved")}</span>
        </div>
      )}
    </div>
  );
}
