import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import PopupWindow from "./popup/PopupWindow";
import NotificationPanel from "./panel/NotificationPanel";

function App() {
  const label = getCurrentWebviewWindow().label;

  if (label.startsWith("popup-")) {
    return <PopupWindow />;
  }

  if (label === "panel") {
    return <NotificationPanel />;
  }

  return null;
}

export default App;
