export type Locale = "zh" | "en";

export const strings: Record<Locale, Record<string, string>> = {
  zh: {
    // Time
    "time.just_now": "刚刚",
    "time.minutes_ago": "{n} 分钟前",
    "time.hours_ago": "{n} 小时前",
    "time.days_ago": "{n} 天前",

    // Status
    "status.running": "运行中",
    "status.pending": "等待中",
    "status.success": "已完成",

    // Panel
    "panel.active_count": "{n} 个活跃",
    "panel.no_active": "无活跃连接",
    "panel.empty": "暂无连接的会话",
    "panel.sessions": "{n} 个会话",
    "panel.delete": "删除",

    // Popup
    "popup.click_to_jump": "点击跳转",

    // Settings
    "settings.title": "设置",
    "settings.sound": "提示音",
    "settings.sound_desc": "会话状态变化时的提示音",
    "settings.mute": "静音",
    "settings.language": "语言",
    "settings.language_desc": "界面显示语言",
    "settings.autostart": "开机自启",
    "settings.autostart_desc": "登录时自动启动 Poke Poke",
    "settings.shortcut": "设置面板快捷键",
    "settings.shortcut_desc": "全局快捷键显示/隐藏设置面板",
    "settings.press_keys": "按下快捷键…",
    "settings.click_to_set": "点击设置",
    "settings.saved": "已保存",
    "settings.preview": "试听",
    "settings.retention": "会话保留",
    "settings.retention_desc": "已完成的会话保留时长，超时自动清理",
    "settings.hours": "{n} 小时",
    "settings.days": "{n} 天",
    "settings.forever": "永久",
  },
  en: {
    // Time
    "time.just_now": "just now",
    "time.minutes_ago": "{n}m ago",
    "time.hours_ago": "{n}h ago",
    "time.days_ago": "{n}d ago",

    // Status
    "status.running": "Running",
    "status.pending": "Pending",
    "status.success": "Completed",

    // Panel
    "panel.active_count": "{n} active",
    "panel.no_active": "No active sessions",
    "panel.empty": "No connected sessions",
    "panel.sessions": "{n} sessions",
    "panel.delete": "Delete",

    // Popup
    "popup.click_to_jump": "Click to jump",

    // Settings
    "settings.title": "Settings",
    "settings.sound": "Alert Sound",
    "settings.sound_desc": "Alert sound on session status changes",
    "settings.mute": "Mute",
    "settings.language": "Language",
    "settings.language_desc": "Display language",
    "settings.autostart": "Launch at Login",
    "settings.autostart_desc": "Start Poke Poke when you log in",
    "settings.shortcut": "Settings Shortcut",
    "settings.shortcut_desc": "Global hotkey to show/hide settings",
    "settings.press_keys": "Press keys…",
    "settings.click_to_set": "Click to set",
    "settings.saved": "Saved",
    "settings.preview": "Preview",
    "settings.retention": "Session Retention",
    "settings.retention_desc": "How long completed sessions are kept before cleanup",
    "settings.hours": "{n}h",
    "settings.days": "{n}d",
    "settings.forever": "Forever",
  },
};
