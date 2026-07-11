import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
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
  });
  const [connected, setConnected] = useState(false);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    // Load config from Rust backend
    invoke<ConfigFile>("get_config")
      .then((cfg) => {
        setConfig(cfg);
        setReady(true);
      })
      .catch((e) => {
        console.error("Failed to load config:", e);
        setReady(true); // still render UI
      });

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
      setConfig(newConfig);
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

  return (
    <div className="app">
      {/* Header with status */}
      <div className="header">
        <div className={`status-dot ${status}`} />
        <span className="header-title">HermesTray</span>
        <span className="header-status">{status}</span>
      </div>

      {/* Tab bar */}
      <div className="tab-bar">
        <div
          className={`tab ${tab === "chat" ? "active" : ""}`}
          onClick={() => setTab("chat")}
        >
          Chat
        </div>
        <div
          className={`tab ${tab === "settings" ? "active" : ""}`}
          onClick={() => setTab("settings")}
        >
          Settings
        </div>
      </div>

      {/* Content */}
      <div className="content">
        {tab === "chat" && <ChatPanel />}
        {tab === "settings" && (
          <SettingsPanel
            config={config}
            onSave={handleSaveConfig}
            onTest={handleTestConnection}
            connected={connected}
            status={status}
          />
        )}
      </div>
    </div>
  );
}

export default App;
