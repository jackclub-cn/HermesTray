import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
  poll_interval_secs_f64?: number;
  toggle_hotkey: string;
  quick_input_hotkey: string;
  language: string;
};

function App() {
  const [tab, setTab] = useState<Tab>("chat");
  const [status, setStatus] = useState<Status>("disconnected");
  const [config, setConfig] = useState<ConfigFile>({
    api_url: "http://localhost:8642",
    api_key: "",
    poll_interval_secs: 3,
    toggle_hotkey: "Alt+Space",
    quick_input_hotkey: "Ctrl+Alt+Shift+C",
    language: "en",
  });
  const [connected, setConnected] = useState(false);
  const [ready, setReady] = useState(false);
  const [lang, setLang] = useState<Lang>("en");

  const t = (key: TKey) => i18n[lang][key] || i18n.en[key] || key;

  // Apply config → set language
  const applyConfig = (cfg: ConfigFile) => {
    setConfig(cfg);
    setLang((cfg.language === "zh" ? "zh" : "en") as Lang);
  };

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

    // Listen for quick-input event (Ctrl+Alt+Shift+C)
    const unlistenQuick = listen("quick-input", () => {
      setTab("chat");
    });

    // Listen for navigate to settings event
    const unlistenNav = listen<string>("navigate", (event) => {
      if (event.payload === "settings") setTab("settings");
    });

    return () => {
      unlistenStatus.then((fn) => fn());
      unlistenQuick.then((fn) => fn());
      unlistenNav.then((fn) => fn());
    };
  }, []);

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
        {tab === "chat" && <ChatPanel lang={lang} onBusyChange={(b) => {
          // ChatPanel self-reports busy state; combine with API connectivity
          if (b) {
            setStatus("busy");
            setConnected(true);
          } else {
            // Restore to whatever the last poll said
            invoke<string>("get_status").then(r => {
              const s = JSON.parse(r) as Status;
              setStatus(s);
              setConnected(s !== "disconnected");
            }).catch(() => {});
          }
        }} />}
        {tab === "settings" && (
          <SettingsPanel
            config={config}
            onSave={handleSaveConfig}
            onTest={handleTestConnection}
            connected={connected}
            status={status}
            lang={lang}
          />
        )}
      </div>
    </div>
  );
}

export default App;