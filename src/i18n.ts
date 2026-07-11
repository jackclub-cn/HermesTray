export type Lang = "en" | "zh";

export type TKey =
  | "tab_chat"
  | "tab_settings"
  | "status_disconnected"
  | "status_idle"
  | "status_busy"
  | "chat_placeholder"
  | "chat_send"
  | "chat_new_session"
  | "chat_hint"
  | "chat_new_session_started"
  | "chat_thinking"
  | "chat_error"
  | "settings_api_url"
  | "settings_api_url_hint"
  | "settings_api_key"
  | "settings_api_key_hint"
  | "settings_poll_interval"
  | "settings_poll_hint"
  | "settings_shortcuts"
  | "settings_toggle_hotkey"
  | "settings_toggle_hint"
  | "settings_quick_input"
  | "settings_quick_input_fixed"
  | "settings_language"
  | "settings_language_hint"
  | "settings_save"
  | "settings_saved"
  | "settings_test"
  | "settings_saved_ok"
  | "settings_live_status"
  | "settings_online"
  | "settings_offline";

export const i18n: Record<Lang, Record<TKey, string>> = {
  en: {
    tab_chat: "Chat",
    tab_settings: "Settings",
    status_disconnected: "disconnected",
    status_idle: "idle",
    status_busy: "busy",
    chat_placeholder: "Type a message...",
    chat_send: "Send",
    chat_new_session: "New session",
    chat_hint: "Press Enter to send, Shift+Enter for newline.",
    chat_new_session_started: "New session started. Press Enter to send.",
    chat_thinking: "Thinking...",
    chat_error: "Error",
    settings_api_url: "API URL",
    settings_api_url_hint: "Hermes API Server address (e.g. http://your-server:8642)",
    settings_api_key: "API Key",
    settings_api_key_hint: "Leave empty if the server doesn't require authentication",
    settings_poll_interval: "Poll Interval (seconds)",
    settings_poll_hint: "How often to check Hermes status",
    settings_shortcuts: "Shortcuts",
    settings_toggle_hotkey: "Toggle Window",
    settings_toggle_hint: "Global shortcut to show/hide the window (e.g. Alt+Space, Ctrl+Alt+H)",
    settings_quick_input: "Quick Input",
    settings_quick_input_fixed: "Ctrl+Alt+Shift+C (fixed)",
    settings_language: "Language",
    settings_language_hint: "Interface language",
    settings_save: "Save",
    settings_saved: "✓ Saved",
    settings_test: "Test Connection",
    settings_saved_ok: "Settings saved successfully",
    settings_live_status: "Live Status",
    settings_online: "Online",
    settings_offline: "Offline",
  },
  zh: {
    tab_chat: "聊天",
    tab_settings: "设置",
    status_disconnected: "已断开",
    status_idle: "待命",
    status_busy: "忙碌",
    chat_placeholder: "输入消息...",
    chat_send: "发送",
    chat_new_session: "新会话",
    chat_hint: "按 Enter 发送，Shift+Enter 换行",
    chat_new_session_started: "新会话已开始，按 Enter 发送",
    chat_thinking: "思考中...",
    chat_error: "错误",
    settings_api_url: "API 地址",
    settings_api_url_hint: "Hermes API 服务地址（如 http://your-server:8642）",
    settings_api_key: "API 密钥",
    settings_api_key_hint: "如果服务器不需要认证则留空",
    settings_poll_interval: "轮询间隔（秒）",
    settings_poll_hint: "检查 Hermes 状态的频率",
    settings_shortcuts: "快捷键",
    settings_toggle_hotkey: "切换窗口",
    settings_toggle_hint: "全局快捷键，显示/隐藏窗口（如 Alt+Space、Ctrl+Alt+H）",
    settings_quick_input: "快速输入",
    settings_quick_input_fixed: "Ctrl+Alt+Shift+C（固定）",
    settings_language: "语言",
    settings_language_hint: "界面语言",
    settings_save: "保存",
    settings_saved: "✓ 已保存",
    settings_test: "测试连接",
    settings_saved_ok: "设置已成功保存",
    settings_live_status: "实时状态",
    settings_online: "在线",
    settings_offline: "离线",
  },
};