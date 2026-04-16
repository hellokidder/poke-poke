import { useEffect, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import PopupWindow from "./popup/PopupWindow";
import SessionPanel from "./panel/SessionPanel";
import SettingsWindow from "./settings/SettingsWindow";
import { LocaleProvider } from "./i18n/context";
import type { Locale } from "./i18n/strings";

function App() {
  const label = getCurrentWebviewWindow().label;
  const [locale, setLocale] = useState<Locale>("zh");

  useEffect(() => {
    invoke<{ locale: string }>("get_settings").then((s) => {
      if (s.locale === "en" || s.locale === "zh") setLocale(s.locale);
    });
    const unlisten = listen("settings-updated", () => {
      invoke<{ locale: string }>("get_settings").then((s) => {
        if (s.locale === "en" || s.locale === "zh") setLocale(s.locale);
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  let content: React.ReactNode = null;

  if (label.startsWith("popup-")) {
    content = <PopupWindow />;
  } else if (label === "panel") {
    content = <SessionPanel />;
  } else if (label === "settings") {
    content = <SettingsWindow />;
  }

  return <LocaleProvider value={locale}>{content}</LocaleProvider>;
}

export default App;
