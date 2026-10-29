import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { sendNotification } from "@tauri-apps/plugin-notification";
import "./App.css";
import { i18n, type Lang, type TKey } from "./i18n";
import { ChatPanel } from "./components/ChatPanel";
import { SettingsPanel } from "./components/SettingsPanel";

type Tab = "chat" | "settings";
type Status = "disconnected" | "idle" | "busy";

export type ConfigFile = {
  api_url: string;
  api_key: string;
  poll_interval_secs: number;
  toggle_hotkey: string;
  quick_input_hotkey: string;
  autostart: boolean;
  language: string;
  theme: string;
};

/** Result of an update check. */
export type UpdateInfo = { version: string; url: string };

function App() {
  const [tab, setTab] = useState<Tab>("chat");
  const [status, setStatus] = useState<Status>("disconnected");
  const [config, setConfig] = useState<ConfigFile>({
    api_url: "http://localhost:8642",
    api_key: "",
    poll_interval_secs: 3,
    toggle_hotkey: "Alt+Space",
    quick_input_hotkey: "Ctrl+Alt+Shift+C",
    autostart: false,
    language: "en",
    theme: "dark",
  });
  const [connected, setConnected] = useState(false);
  const [ready, setReady] = useState(false);
  const [lang, setLang] = useState<Lang>("en");
  const lastStatusRef = useRef<Status>("disconnected");

  const t = useCallback(
    (key: TKey) => i18n[lang][key] || i18n.en[key] || key,
    [lang]
  );

  // Apply config → language + theme
  const applyConfig = (cfg: ConfigFile) => {
    setConfig(cfg);
    setLang((cfg.language === "zh" ? "zh" : "en") as Lang);
    document.documentElement.dataset.theme = cfg.theme === "light" ? "light" : "dark";
  };

  // Check for updates against GitHub Releases. Returns human-readable result.
  const runUpdateCheck = useCallback(async (): Promise<string> => {
    try {
      const info = await invoke<UpdateInfo | null>("check_update");
      if (info && info.url) {
        return `${t("settings_update_available")} v${info.version} — ${t("chat_export")}: ${info.url}`;
      }
      return t("settings_update_latest");
    } catch (e) {
      return `${t("settings_update_failed")}: ${e}`;
    }
  }, [t]);

  useEffect(() => {
    // Load config from Rust backend
    invoke<ConfigFile>("get_config")
      .then((cfg) => {
        applyConfig(cfg);
        setReady(true);
      })
      .catch((e) => {
        console.error("Failed to load config:", e);
        setReady(true);
      });

    // Query current status immediately (don't wait for first status-change event)
    invoke<string>("get_status")
      .then((result) => {
        const parsed = JSON.parse(result) as Status;
        setStatus(parsed);
        setConnected(parsed !== "disconnected");
      })
      .catch((e) => console.error("Failed to get initial status:", e));

    // Listen for status events from Rust (background polling)
    const unlistenStatus = listen<{ status: Status }>("hermes-status", (event) => {
      setStatus(event.payload.status);
      setConnected(event.payload.status !== "disconnected");
    });

    // Listen for quick-input event
    const unlistenQuick = listen("quick-input", () => {
      setTab("chat");
    });

    // Listen for navigate to settings event
    const unlistenNav = listen<string>("navigate", (event) => {
      if (event.payload === "settings") setTab("settings");
    });

    // Tray menu: New Chat
    const unlistenNewSession = listen("new-session", () => {
      setTab("chat");
    });

    // Tray menu: Check for Updates
    const unlistenCheckUpdate = listen("check-update", () => {
      runUpdateCheck().then((msg) => {
        sendNotification({ title: "HermesTray", body: msg });
      });
    });

    // Esc hides the window (skip while capturing a hotkey in Settings)
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      const el = document.activeElement as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA") && el.hasAttribute("readonly")) {
        return; // hotkey capture field
      }
      invoke("hide_window").catch(() => {});
    };
    window.addEventListener("keydown", onKeyDown);

    return () => {
      unlistenStatus.then((fn) => fn());
      unlistenQuick.then((fn) => fn());
      unlistenNav.then((fn) => fn());
      unlistenNewSession.then((fn) => fn());
      unlistenCheckUpdate.then((fn) => fn());
      window.removeEventListener("keydown", onKeyDown);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Notify once when the connection drops
  useEffect(() => {
    if (status === "disconnected" && lastStatusRef.current !== "disconnected") {
      sendNotification({
        title: "HermesTray",
        body: t("status_notification_disconnected"),
      });
    }
    lastStatusRef.current = status;
  }, [status, t]);

  const handleSaveConfig = useCallback(async (newConfig: ConfigFile) => {
    try {
      await invoke("update_config", { newConfig });
      applyConfig(newConfig);
      return true;
    } catch (e) {
      console.error("Failed to save config", e);
      return false;
    }
  }, []);

  const handleTestConnection = useCallback(async () => {
    try {
      const result = await invoke<string>("test_connection");
      return result;
    } catch (e) {
      return String(e);
    }
  }, []);

  const setStatusDotClass = (s: Status) => {
    if (s === "idle") return "connected";
    return s;
  };

  if (!ready) {
    return (
      <div className="app">
        <div className="header">
          <div className={`status-dot disconnected`} />
          <span className="header-title">HermesTray</span>
          <span className="header-status">loading...</span>
        </div>
        <div className="content" style={{ display: "flex", alignItems: "center", justifyContent: "center" }}>
          <span className="loading">Connecting to Hermes...</span>
        </div>
      </div>
    );
  }

  const statusText = (s: Status) => {
    const map: Record<Status, TKey> = {
      disconnected: "status_disconnected",
      idle: "status_idle",
      busy: "status_busy",
    };
    return t(map[s]);
  };

  return (
    <div className="app">
      {/* Header with status */}
      <div className="header">
        <div className={`status-dot ${setStatusDotClass(status)}`} />
        <span className="header-title">HermesTray</span>
        <span className="header-status">{statusText(status)}</span>
        {/* Language toggle */}
        <button
          className="lang-toggle"
          onClick={() => {
            const newLang: Lang = lang === "en" ? "zh" : "en";
            const newCfg = { ...config, language: newLang };
            invoke("update_config", { newConfig: newCfg }).catch(() => {});
            setLang(newLang);
          }}
          title={lang === "en" ? "切换到中文" : "Switch to English"}
        >
          {lang === "en" ? "中" : "EN"}
        </button>
      </div>

      {/* Tab bar */}
      <div className="tab-bar">
        <div
          className={`tab ${tab === "chat" ? "active" : ""}`}
          onClick={() => setTab("chat")}
        >
          {t("tab_chat")}
        </div>
        <div
          className={`tab ${tab === "settings" ? "active" : ""}`}
          onClick={() => setTab("settings")}
        >
          {t("tab_settings")}
        </div>
      </div>

      {/* Content */}
      <div className="content">
        <div style={{ display: tab === "chat" ? "flex" : "none", flex: 1, overflow: "hidden", flexDirection: "column" }}>
          <ChatPanel lang={lang} />
        </div>
        <div style={{ display: tab === "settings" ? "block" : "none", flex: 1, overflow: "auto" }}>
          <SettingsPanel
            config={config}
            onSave={handleSaveConfig}
            onTest={handleTestConnection}
            onCheckUpdate={runUpdateCheck}
            connected={connected}
            status={status}
            lang={lang}
          />
        </div>
      </div>
    </div>
  );
}

export default App;
